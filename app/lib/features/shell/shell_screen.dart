/// The shell: header, rail, sections, status bar, palette and the shortcuts
/// that bind them (HUM-019).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/flow_handoff.dart';
import '../../core/shortcuts/intents.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import '../audit/audit_screen.dart';
import '../history/history_screen.dart';
import '../intercept/intercept_screen.dart';
import '../intercept/providers/decision.dart';
import '../intercept/providers/flows.dart';
import '../rules/rules_screen.dart';
import '../sandbox/sandbox_screen.dart';
import 'providers/connection.dart';
import 'providers/navigation.dart';
import 'providers/theme.dart';
import 'section.dart';
import 'widgets/command_palette.dart';
import 'widgets/header_bar.dart';
import 'widgets/icon_rail.dart';
import 'widgets/shell_notices.dart';
import 'widgets/status_bar.dart';

/// The shell, shown once the daemon answered.
class ShellScreen extends ConsumerStatefulWidget {
  /// Creates the shell for the connected [info].
  const ShellScreen({required this.info, super.key});

  /// What `GetInfo` said; the status bar shows it.
  final DaemonInfo info;

  @override
  ConsumerState<ShellScreen> createState() => _ShellScreenState();
}

class _ShellScreenState extends ConsumerState<ShellScreen> {
  final FocusNode _focus = FocusNode(debugLabel: 'shell');
  bool _paletteOpen = false;

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  void _openPalette() => setState(() => _paletteOpen = true);

  void _closePalette() {
    setState(() => _paletteOpen = false);
    // The palette's field held the focus; without this the next Ctrl+2 would
    // reach nobody.
    _focus.requestFocus();
  }

  void _togglePalette() => _paletteOpen ? _closePalette() : _openPalette();

  List<PaletteCommand> _commands(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    final Navigation navigation = ref.read(navigationProvider.notifier);
    return <PaletteCommand>[
      for (final Section section in Section.values)
        PaletteCommand(
          id: 'go-${section.name}',
          label: l10n.shellPaletteGoTo(section.label(l10n)),
          run: () => navigation.go(section),
        ),
      PaletteCommand(
        id: 'toggle-theme',
        label: l10n.shellPaletteToggleTheme,
        run: () => ref
            .read(themeModeProvider.notifier)
            .toggle(MediaQuery.platformBrightnessOf(context)),
      ),
      PaletteCommand(
        id: 'reconnect',
        label: l10n.shellPaletteReconnect,
        run: ref.read(connectionStateProvider.notifier).retry,
      ),
      // The one "allow all" of the program lives here and nowhere else: no
      // control on the queue carries that label, and this command opens the
      // modal with the hosts instead of sending anything (HUM-029).
      PaletteCommand(
        id: 'queue-allow-all',
        label: l10n.paletteQueueAllowAll,
        run: () {
          navigation.go(Section.intercept);
          ref.read(interceptDecisionProvider.notifier).askAllowAll();
        },
      ),
    ];
  }

  /// Carries out a handover another section asked for.
  ///
  /// The history hands a held request to the queue; a feature may not reach
  /// into another feature to do it, and it does not have to — the shell is
  /// what composes the sections (ARCHITECTURE 5). The note is cleared here,
  /// so it is carried out once.
  void _takeHandoff(FlowId? previous, FlowId? next) {
    if (next == null) {
      return;
    }
    ref.read(selectedFlowIdProvider.notifier).select(next);
    ref.read(navigationProvider.notifier).go(Section.intercept);
    ref.read(flowHandoffProvider.notifier).clear();
  }

  @override
  Widget build(BuildContext context) {
    ref.listen<FlowId?>(flowHandoffProvider, _takeHandoff);
    final Section section = ref.watch(navigationProvider);
    final Navigation navigation = ref.read(navigationProvider.notifier);
    final HTokens tokens = HTheme.of(context);
    return Shortcuts(
      shortcuts: shellShortcuts(),
      child: Actions(
        actions: <Type, Action<Intent>>{
          NavIntent: CallbackAction<NavIntent>(
            onInvoke: (NavIntent intent) {
              navigation.goIndex(intent.index);
              return null;
            },
          ),
          PaletteIntent: CallbackAction<PaletteIntent>(
            onInvoke: (PaletteIntent intent) {
              _togglePalette();
              return null;
            },
          ),
        },
        child: Focus(
          focusNode: _focus,
          autofocus: true,
          child: Stack(
            fit: StackFit.expand,
            children: <Widget>[
              ColoredBox(
                color: tokens.colors.bg0,
                child: Column(
                  children: <Widget>[
                    HeaderBar(section: section, onPalette: _togglePalette),
                    const ShellNotices(),
                    Expanded(
                      child: Row(
                        children: <Widget>[
                          IconRail(active: section, onSelect: navigation.go),
                          Expanded(
                            child: IndexedStack(
                              index: section.index,
                              children: const <Widget>[
                                InterceptScreen(),
                                HistoryScreen(),
                                RulesScreen(),
                                SandboxScreen(),
                                AuditScreen(),
                              ],
                            ),
                          ),
                        ],
                      ),
                    ),
                    StatusBar(info: widget.info),
                  ],
                ),
              ),
              if (_paletteOpen)
                CommandPalette(
                  commands: _commands(context),
                  onClose: _closePalette,
                ),
            ],
          ),
        ),
      ),
    );
  }
}
