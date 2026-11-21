/// The start window (HUM-537, ADR-0024 point 5): the first screen whenever
/// the window shows no session.
///
/// Three tabs on a rail at the left, like the welcome screen of an IDE:
/// projects (running, recent, new), cleanup, and the global settings. The
/// settings are the same component as the sheet in a session; the shell
/// hands the tab in as [StartScreen.settings], because no feature imports
/// another one (ARCHITECTURE 5, `tools/check-deps.sh`).
///
/// Nothing on this screen starts a session without a project: a start names
/// the folder of a recent project or of the folder just chosen, and the
/// window only changes to the session view once the daemon reports the
/// session running (`core/ui/session_view.dart`).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ui/app_mark.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'providers/cleanup.dart';
import 'providers/first_start.dart';
import 'widgets/cleanup_pane.dart';
import 'widgets/project_list.dart';

/// The tabs of the start window.
enum StartTab {
  /// Running, recent and new projects.
  projects,

  /// Cleanup of recorded data.
  cleanup,

  /// The global settings.
  settings,
}

/// Width of the rail with the tabs.
const double startRailWidth = 224;

/// Builds the question on cloud inference (`CloudConfirmDialog`) for the
/// first-start screen: [onCancel] closes it, [onConfirm] confirms and
/// starts.
typedef CloudConfirmBuilder = Widget Function(
  CloudInference cloud,
  VoidCallback onCancel,
  VoidCallback onConfirm,
);

/// The start window.
class StartScreen extends ConsumerStatefulWidget {
  /// Creates the window with [settings] as the content of its settings tab.
  const StartScreen({
    required this.settings,
    required this.cloudConfirm,
    this.initialTab = StartTab.projects,
    super.key,
  });

  /// The settings tab (`SettingsTab`), built by the shell.
  final Widget settings;

  /// The question on cloud inference, built by the shell: it belongs to the
  /// sandbox feature, and no feature imports another one (HUM-538).
  final CloudConfirmBuilder cloudConfirm;

  /// The tab shown first.
  final StartTab initialTab;

  @override
  ConsumerState<StartScreen> createState() => _StartScreenState();
}

class _StartScreenState extends ConsumerState<StartScreen> {
  late StartTab _tab = widget.initialTab;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final CleanupPlan? plan = ref.watch(
      cleanupProvider.select((CleanupState s) => s.plan),
    );
    final FirstStart? firstStart = ref.watch(firstStartProvider);
    final CloudInference? cloud = firstStart?.proposal?.cloudInference;
    return Stack(
      children: <Widget>[
        Positioned.fill(
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              _StartRail(
                selected: _tab,
                onSelect: (StartTab tab) => setState(() => _tab = tab),
              ),
              HHairline(vertical: true, color: tokens.colors.line),
              Expanded(
                child: switch (_tab) {
                  StartTab.projects => const ProjectsPane(),
                  StartTab.cleanup => const CleanupPane(),
                  StartTab.settings => KeyedSubtree(
                    key: const Key('start-settings'),
                    child: widget.settings,
                  ),
                },
              ),
            ],
          ),
        ),
        if (plan != null) Positioned.fill(child: CleanupConfirm(plan: plan)),
        if (cloud != null && (firstStart?.askingCloud ?? false))
          Positioned.fill(
            child: widget.cloudConfirm(
              cloud,
              ref.read(firstStartProvider.notifier).dismissCloud,
              () => unawaited(
                ref.read(firstStartProvider.notifier).confirmCloud(),
              ),
            ),
          ),
      ],
    );
  }
}

/// The rail: the mark and the product name over the three tabs.
class _StartRail extends StatelessWidget {
  const _StartRail({required this.selected, required this.onSelect});

  final StartTab selected;
  final ValueChanged<StartTab> onSelect;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return SizedBox(
      width: startRailWidth,
      child: ColoredBox(
        color: tokens.colors.bg1,
        child: Padding(
          padding: EdgeInsets.all(tokens.spacing.x4),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Padding(
                padding: EdgeInsets.symmetric(
                  horizontal: tokens.spacing.x2,
                  vertical: tokens.spacing.x2,
                ),
                child: Row(
                  children: <Widget>[
                    const AppMark(size: 28),
                    SizedBox(width: tokens.spacing.x3),
                    Text(
                      l10n.appTitle,
                      style: tokens.typography.ui16
                          .copyWith(fontWeight: FontWeight.w600)
                          .tinted(tokens.colors.fg0),
                    ),
                  ],
                ),
              ),
              SizedBox(height: tokens.spacing.x5),
              for (final (StartTab tab, String label) in <(StartTab, String)>[
                (StartTab.projects, l10n.startNavProjects),
                (StartTab.cleanup, l10n.startNavCleanup),
                (StartTab.settings, l10n.startNavSettings),
              ])
                Padding(
                  padding: EdgeInsets.only(bottom: tokens.spacing.x1),
                  child: _RailItem(
                    key: Key('start-tab-${tab.name}'),
                    label: label,
                    selected: tab == selected,
                    onTap: () => onSelect(tab),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

/// One tab of the rail: the selected one on a raised surface with a bar of
/// the brand colour at its left edge.
class _RailItem extends StatefulWidget {
  const _RailItem({
    required this.label,
    required this.selected,
    required this.onTap,
    super.key,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  State<_RailItem> createState() => _RailItemState();
}

class _RailItemState extends State<_RailItem> {
  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool selected = widget.selected;
    final Color fill = selected
        ? tokens.colors.bg3
        : (_hovered ? tokens.colors.bg2 : const Color(0x00000000));
    return Semantics(
      button: true,
      selected: selected,
      label: widget.label,
      excludeSemantics: true,
      child: FocusableActionDetector(
        onShowHoverHighlight: (bool on) => setState(() => _hovered = on),
        onShowFocusHighlight: (bool on) => setState(() => _focused = on),
        mouseCursor: SystemMouseCursors.click,
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent _) {
              widget.onTap();
              return null;
            },
          ),
        },
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onTap,
          child: Container(
            height: 32,
            decoration: BoxDecoration(
              color: fill,
              borderRadius: BorderRadius.circular(tokens.radii.control),
              border: _focused ? Border.all(color: tokens.colors.accent) : null,
            ),
            child: Row(
              children: <Widget>[
                Container(
                  width: 3,
                  height: 16,
                  margin: EdgeInsets.only(right: tokens.spacing.x2),
                  decoration: BoxDecoration(
                    color: selected
                        ? tokens.colors.accent
                        : const Color(0x00000000),
                    borderRadius: BorderRadius.circular(2),
                  ),
                ),
                Expanded(
                  child: Text(
                    widget.label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: tokens.typography.ui13
                        .copyWith(
                          fontWeight: selected
                              ? FontWeight.w600
                              : FontWeight.w400,
                        )
                        .tinted(
                          selected ? tokens.colors.fg0 : tokens.colors.fg1,
                        ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
