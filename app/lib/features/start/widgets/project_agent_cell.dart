/// The agent column of a project row in the start window (GitHub #111).
///
/// The agent of the last start, and under it, for a depot agent, the state
/// of its depot as `Agents` says: the installed version, "Needs setup"
/// while install or login are missing, "Updating…" while an update runs.
/// The cell is the way to the guided setup of the project: a click opens it
/// at the first step that needs a hand, or at "Keep up to date" when none
/// does.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/agents_provider.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/agent_setup.dart';
import '../setup/setup_steps.dart';

/// The cell.
class ProjectAgentCell extends ConsumerStatefulWidget {
  /// Creates the cell of [entry]; [enabled] false shows it without the way
  /// to the setup, [dim] in the quiet colour of a project that cannot
  /// start.
  const ProjectAgentCell({
    required this.entry,
    required this.enabled,
    required this.dim,
    super.key,
  });

  /// The project.
  final ProjectEntry entry;

  /// Whether a click opens the setup.
  final bool enabled;

  /// Whether the project is shown quiet.
  final bool dim;

  @override
  ConsumerState<ProjectAgentCell> createState() => _ProjectAgentCellState();
}

class _ProjectAgentCellState extends ConsumerState<ProjectAgentCell> {
  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final ProjectEntry entry = widget.entry;
    final AgentDepot? depot = ref.watch(
      agentsProvider.select(
        (AsyncValue<AgentsStatus> s) => s.value?.of(entry.lastAgent),
      ),
    );
    final bool enabled = widget.enabled && entry.lastAgent.isNotEmpty;
    final Widget? state = _depotLine(tokens, l10n, depot);
    final Widget column = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Text(
          entry.lastAgent,
          key: Key('start-agent-${entry.projectId}'),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: tokens.typography.ui12.tinted(
            widget.dim ? tokens.colors.fg2 : tokens.colors.fg1,
          ),
        ),
        ?state,
      ],
    );
    if (!enabled) {
      return column;
    }
    final String name = depot?.displayName ?? entry.lastAgent;
    return Semantics(
      button: true,
      label: l10n.startAgentSetup(name),
      child: FocusableActionDetector(
        onShowHoverHighlight: (bool on) => setState(() => _hovered = on),
        onShowFocusHighlight: (bool on) => setState(() => _focused = on),
        mouseCursor: SystemMouseCursors.click,
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent _) {
              _open(depot);
              return null;
            },
          ),
        },
        child: GestureDetector(
          key: Key('start-agent-setup-${entry.projectId}'),
          behavior: HitTestBehavior.opaque,
          onTap: () => _open(depot),
          child: Transform.translate(
            offset: Offset(-tokens.spacing.x2, 0),
            child: HFocusRing.inline(
              visible: _focused,
              radius: tokens.radii.control,
              child: Container(
                constraints: BoxConstraints(minHeight: tokens.sizes.hitMin),
                // Der Text bleibt an der Kante der Spalte; die Fläche beim
                // Überfahren ragt links und rechts um denselben Rand hinaus.
                padding: EdgeInsets.symmetric(
                  horizontal: tokens.spacing.x2,
                  vertical: tokens.spacing.x1,
                ),
                decoration: BoxDecoration(
                  color: _hovered ? tokens.colors.bg2 : const Color(0x00000000),
                  borderRadius: BorderRadius.circular(tokens.radii.control),
                ),
                alignment: Alignment.centerLeft,
                child: column,
              ),
            ),
          ),
        ),
      ),
    );
  }

  /// The state of the depot in one short line; null for a host agent.
  Widget? _depotLine(HTokens tokens, AppLocalizations l10n, AgentDepot? depot) {
    if (depot == null || depot.source != AgentSource.depot) {
      return null;
    }
    final ProjectEntry entry = widget.entry;
    final AgentMaintenance? run = depot.maintenance;
    if (run != null && run.running) {
      return _line(
        tokens,
        Key('start-agent-state-${entry.projectId}'),
        HGlyph.hourglass,
        tokens.colors.fg1,
        l10n.startAgentRunning,
        tokens.typography.ui11.tinted(tokens.colors.fg1),
      );
    }
    if (depotNeedsSetup(depot)) {
      return _line(
        tokens,
        Key('start-agent-state-${entry.projectId}'),
        HGlyph.triangleAlert,
        tokens.colors.warning,
        l10n.startAgentNeedsSetup,
        tokens.typography.ui11.tinted(tokens.colors.warningText),
      );
    }
    return _line(
      tokens,
      Key('start-agent-state-${entry.projectId}'),
      null,
      tokens.colors.fg2,
      l10n.startAgentDepotVersion(depot.version),
      tokens.typography.mono11.tinted(tokens.colors.fg2),
    );
  }

  static Widget _line(
    HTokens tokens,
    Key key,
    HGlyph? glyph,
    Color color,
    String text,
    TextStyle style,
  ) => Row(
    key: key,
    mainAxisSize: MainAxisSize.min,
    children: <Widget>[
      if (glyph != null) ...<Widget>[
        HGlyphIcon(glyph, size: 11, color: color),
        SizedBox(width: tokens.spacing.x1),
      ],
      Flexible(
        child: Text(
          text,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: style,
        ),
      ),
    ],
  );

  void _open(AgentDepot? depot) {
    final bool current =
        depot != null &&
        depot.source == AgentSource.depot &&
        !depotNeedsSetup(depot);
    unawaited(
      ref
          .read(agentSetupProvider.notifier)
          .open(
            AgentSetupTarget.of(widget.entry),
            step: current ? SetupStepId.update : null,
          ),
    );
  }
}
