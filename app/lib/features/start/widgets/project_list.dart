/// The projects tab of the start window: running, recent and new
/// (HUM-537).
///
/// One card per group, one row per project, and every row the same columns:
/// a tile with the initial, name over path, agent, time, "Start choice…"
/// and one button at the right edge. The columns have fixed widths so that
/// agent, time and buttons stand under each other from row to row.
///
/// "New project" opens the first-start screen for the chosen folder in
/// place of the lists (HUM-538); a folder the register knows starts without
/// it. "Start choice…" opens the same screen for a recent project.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/time/now.dart';
import '../../../core/ui/diagnostic_severity.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../core/ui/untranslated_wording.dart';
import '../../../core/ui/work_dir_picker.dart';
import '../../../l10n/l10n.dart';
import '../providers/first_start.dart';
import '../providers/projects.dart';
import '../start_text.dart';
import 'first_start_pane.dart';
import 'start_page.dart';

/// Width of the agent column.
const double _agentColumn = 112;

/// Width of the time column.
const double _timeColumn = 104;

/// Height of the name line, the height of a badge.
const double _nameLine = 26;

/// Width of the button column.
const double _buttonColumn = 104;

/// Width added to the button column for "Start choice…"; on a running
/// project it stays empty, so the other columns stand under each other in
/// both cards.
const double _choiceColumn = 120;

/// The projects tab.
class ProjectsPane extends ConsumerWidget {
  /// Creates the tab.
  const ProjectsPane({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AsyncValue<List<ProjectEntry>> projects = ref.watch(projectsProvider);
    final StartLaunch launch = ref.watch(startLaunchProvider);
    // Der Erststart-Bildschirm steht an der Stelle der Liste (HUM-538).
    final FirstStart? firstStart = ref.watch(firstStartProvider);
    if (firstStart != null) {
      return FirstStartPane(state: firstStart);
    }
    final List<ProjectEntry> all = projects.value ?? const <ProjectEntry>[];
    final List<ProjectEntry> running = runningProjects(all);
    final List<ProjectEntry> recent = recentProjects(all);
    return StartPage(
      key: const Key('start-projects'),
      title: l10n.startNavProjects,
      action: HButton(
        key: const Key('start-new-project'),
        variant: HButtonVariant.primary,
        leading: HGlyphIcon(
          HGlyph.plus,
          size: 14,
          color: tokens.colors.onAccent,
        ),
        onPressed: launch.busyPath != null
            ? null
            : () => unawaited(
                chooseWorkDir(
                  ref,
                  (String path) => unawaited(
                    ref.read(firstStartProvider.notifier).openFolder(path),
                  ),
                ),
              ),
        child: Text(l10n.startNewProject),
      ),
      children: <Widget>[
        for (final Diagnostic diagnostic in <Diagnostic>[
          ?ref.watch(folderDialogFailureProvider),
          ...launch.diagnostics,
        ])
          Padding(
            padding: EdgeInsets.only(bottom: tokens.spacing.x3),
            child: StartDiagnostic(diagnostic: diagnostic),
          ),
        if (launch.endedSilently)
          Padding(
            padding: EdgeInsets.only(bottom: tokens.spacing.x3),
            child: Text(
              l10n.startEndedSilently,
              key: const Key('start-ended-silently'),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
          ),
        switch (projects) {
          AsyncError(:final Object error) => _ListFailed(error: error),
          AsyncData() when all.isEmpty => _Empty(text: l10n.startEmpty),
          AsyncData() => Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (running.isNotEmpty) ...<Widget>[
                StartSectionLabel(text: l10n.startRunningTitle),
                _ProjectCard(
                  key: const Key('start-running'),
                  entries: running,
                  busyPath: launch.busyPath,
                ),
                SizedBox(height: tokens.spacing.x5),
              ],
              if (recent.isNotEmpty) ...<Widget>[
                StartSectionLabel(text: l10n.startRecentTitle),
                _ProjectCard(
                  key: const Key('start-recent'),
                  entries: recent,
                  busyPath: launch.busyPath,
                ),
              ],
            ],
          ),
          _ => const HSkeleton(rows: 3),
        },
      ],
    );
  }
}

/// A finding of the daemon as a card the width of the page.
class StartDiagnostic extends StatelessWidget {
  /// Creates the card.
  const StartDiagnostic({required this.diagnostic, super.key});

  /// What the daemon said.
  final Diagnostic diagnostic;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DiagnosticText text = DiagnosticL10n.resolve(diagnostic, l10n);
    return HDiagnosticCard(
      key: Key('start-diagnostic-${diagnostic.code}'),
      code: diagnostic.code,
      severityLabel: severityLabel(l10n, diagnostic.severity),
      color: severityColor(tokens, diagnostic.severity),
      title: text.title,
      why: text.cause,
      original: UntranslatedWording.forDiagnostic(diagnostic, l10n),
      docsUrl: diagnostic.docsUrl,
      width: double.infinity,
    );
  }
}

class _ListFailed extends ConsumerWidget {
  const _ListFailed({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        if (error case DaemonException(:final Diagnostic diagnostic))
          StartDiagnostic(diagnostic: diagnostic)
        else
          Text(
            '$error',
            style: tokens.typography.ui13.tinted(tokens.colors.fg1),
          ),
        SizedBox(height: tokens.spacing.x3),
        HButton(
          key: const Key('start-retry'),
          onPressed: () =>
              unawaited(ref.read(projectsProvider.notifier).reload()),
          child: Text(context.l10n.startRetry),
        ),
      ],
    );
  }
}

class _Empty extends StatelessWidget {
  const _Empty({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return StartCard(
      child: Padding(
        padding: EdgeInsets.symmetric(
          horizontal: tokens.spacing.x4,
          vertical: tokens.spacing.x6,
        ),
        child: Text(
          text,
          key: const Key('start-empty'),
          textAlign: TextAlign.center,
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
      ),
    );
  }
}

/// One group of projects in a card, rows divided by hairlines.
class _ProjectCard extends StatelessWidget {
  const _ProjectCard({
    required this.entries,
    required this.busyPath,
    super.key,
  });

  final List<ProjectEntry> entries;
  final String? busyPath;

  @override
  Widget build(BuildContext context) => StartCard(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final (int index, ProjectEntry entry) in entries.indexed) ...[
          if (index > 0) const HHairline(),
          _ProjectRow(entry: entry, busy: busyPath == entry.path),
        ],
      ],
    ),
  );
}

class _ProjectRow extends ConsumerWidget {
  const _ProjectRow({required this.entry, required this.busy});

  final ProjectEntry entry;
  final bool busy;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DateTime now = ref.watch(
      // Auf die Minute: Die Zeile baut sich einmal je Minute neu, nicht
      // viermal je Sekunde (`docs/UX.md` 7).
      nowProvider.select(
        (DateTime t) => DateTime.fromMillisecondsSinceEpoch(
          t.millisecondsSinceEpoch ~/
              Duration.millisecondsPerMinute *
              Duration.millisecondsPerMinute,
          isUtc: t.isUtc,
        ),
      ),
    );
    final bool dim = !entry.running && !entry.startable;
    final Color primary = dim ? tokens.colors.fg2 : tokens.colors.fg0;
    final StartLaunchNotifier launch = ref.read(startLaunchProvider.notifier);
    // Ein Daemon vor 1.58 startet kein Projekt an seiner Kennung (HUM-537);
    // solange `GetInfo` aussteht, bleibt der Knopf an, und der Start wartet.
    final DaemonInfo? info = ref.watch(daemonInfoProvider).value;
    final bool oldDaemon =
        !entry.running && info != null && !info.startsByProjectId;
    final Widget button = entry.running
        ? HButton(
            key: Key('start-attach-${entry.projectId}'),
            size: HButtonSize.sm,
            onPressed: () => launch.attach(entry),
            child: Text(l10n.startAttach),
          )
        : HButton(
            key: Key('start-start-${entry.projectId}'),
            size: HButtonSize.sm,
            onPressed: entry.startable && !busy && !oldDaemon
                ? () => unawaited(launch.startRecent(entry))
                : null,
            child: Text(busy ? l10n.startStarting : l10n.startStart),
          );
    return Padding(
      key: Key('start-project-${entry.projectId}'),
      padding: EdgeInsets.symmetric(
        horizontal: tokens.spacing.x4,
        vertical: tokens.spacing.x3,
      ),
      child: Row(
        children: <Widget>[
          _InitialTile(entry: entry, dim: dim),
          SizedBox(width: tokens.spacing.x3),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                // Eine feste Höhe für die Namenszeile: Mit und ohne Badge
                // sind alle Zeilen der Karte gleich hoch.
                SizedBox(
                  height: _nameLine,
                  child: Row(
                    children: <Widget>[
                      Flexible(
                        child: Text(
                          entry.displayName,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: tokens.typography.ui13
                              .copyWith(fontWeight: FontWeight.w600)
                              .tinted(primary),
                        ),
                      ),
                      if (entry.missing) ...<Widget>[
                        SizedBox(width: tokens.spacing.x2),
                        HBadge(
                          key: Key('start-missing-${entry.projectId}'),
                          text: l10n.startMissing,
                          color: tokens.colors.warning,
                        ),
                      ],
                      if (entry.changesWaiting) ...<Widget>[
                        SizedBox(width: tokens.spacing.x2),
                        HBadge(
                          text: l10n.startChangesWaiting,
                          color: tokens.state.held,
                        ),
                      ],
                    ],
                  ),
                ),
                HMiddleEllipsis(
                  entry.path,
                  style: tokens.typography.mono11.tinted(tokens.colors.fg2),
                ),
              ],
            ),
          ),
          SizedBox(width: tokens.spacing.x4),
          SizedBox(
            width: _agentColumn,
            child: Text(
              entry.lastAgent,
              key: Key('start-agent-${entry.projectId}'),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: tokens.typography.ui12.tinted(
                dim ? tokens.colors.fg2 : tokens.colors.fg1,
              ),
            ),
          ),
          SizedBox(
            width: _timeColumn,
            child: Text(
              startAge(l10n, entry.lastStarted, now),
              key: Key('start-age-${entry.projectId}'),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            ),
          ),
          // „Startwahl…" und der Knopf stehen rechtsbündig in einer Spalte
          // fester Breite: Agent und Zeit fluchten so in beiden Karten, und
          // zwischen den beiden Knöpfen bleibt nur der übliche Abstand.
          SizedBox(
            width: _choiceColumn + _buttonColumn,
            child: Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: <Widget>[
                if (!entry.running) ...<Widget>[
                  // Schmal geworden (große Schrift, längere Übersetzung)
                  // kürzt sich die Beschriftung, der Startknopf nie.
                  Flexible(
                    child: HButton(
                      key: Key('start-choice-${entry.projectId}'),
                      variant: HButtonVariant.ghost,
                      semanticsLabel: l10n.firstStartChangeChoiceLabel(
                        entry.displayName,
                      ),
                      onPressed: entry.startable && !busy && !oldDaemon
                          ? () => unawaited(
                              ref
                                  .read(firstStartProvider.notifier)
                                  .openProject(entry),
                            )
                          : null,
                      child: Text(
                        l10n.firstStartChangeChoice,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                ],
                if (entry.missing || oldDaemon)
                  HoverLabel(
                    label: entry.missing
                        ? l10n.startMissingWhy
                        : l10n.startDaemonTooOld,
                    side: HoverLabelSide.below,
                    child: button,
                  )
                else
                  button,
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// The initial of the folder on a tile; a green dot at its corner while a
/// session runs in it.
class _InitialTile extends StatelessWidget {
  const _InitialTile({required this.entry, required this.dim});

  final ProjectEntry entry;
  final bool dim;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String name = entry.displayName.trim();
    final String initial = name.isEmpty
        ? '?'
        : String.fromCharCode(name.runes.first).toUpperCase();
    return SizedBox.square(
      dimension: 32,
      child: Stack(
        clipBehavior: Clip.none,
        children: <Widget>[
          Positioned.fill(
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: tokens.colors.bg3,
                borderRadius: BorderRadius.circular(tokens.radii.control),
                border: Border.all(color: tokens.colors.line),
              ),
              child: Center(
                child: Text(
                  initial,
                  style: tokens.typography.ui14
                      .copyWith(fontWeight: FontWeight.w600)
                      .tinted(dim ? tokens.colors.fg2 : tokens.colors.fg1),
                ),
              ),
            ),
          ),
          if (entry.running)
            Positioned(
              right: -2,
              bottom: -2,
              child: Container(
                key: Key('start-running-dot-${entry.projectId}'),
                width: 10,
                height: 10,
                decoration: BoxDecoration(
                  color: tokens.state.allowed,
                  shape: BoxShape.circle,
                  border: Border.all(color: tokens.colors.bg1, width: 2),
                ),
              ),
            ),
        ],
      ),
    );
  }
}
