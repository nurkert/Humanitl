/// The cleanup tab of the start window (HUM-537): the selection, the button
/// that asks the daemon for the plan, and the result of the last cleanup.
/// The plan itself shows in [CleanupConfirm].
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/text/format.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/cleanup.dart';
import '../providers/projects.dart';
import 'project_list.dart' show StartDiagnostic;
import 'start_page.dart';

/// The cleanup tab.
class CleanupPane extends ConsumerWidget {
  /// Creates the tab.
  const CleanupPane({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final CleanupState cleanup = ref.watch(cleanupProvider);
    final CleanupNotifier notifier = ref.read(cleanupProvider.notifier);
    final List<ProjectEntry> projects =
        ref.watch(projectsProvider).value ?? const <ProjectEntry>[];
    final CleanupResult? result = cleanup.result;
    final Diagnostic? diagnostic = cleanup.diagnostic;
    return StartPage(
      key: const Key('start-cleanup'),
      title: l10n.startNavCleanup,
      intro: l10n.startCleanupIntro,
      children: <Widget>[
        if (diagnostic != null)
          Padding(
            padding: EdgeInsets.only(bottom: tokens.spacing.x3),
            child: StartDiagnostic(diagnostic: diagnostic),
          ),
        StartCard(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (projects.isEmpty)
                _Line(
                  child: Text(
                    l10n.startCleanupNoProjects,
                    style: tokens.typography.ui13.tinted(tokens.colors.fg1),
                  ),
                ),
              for (final (int index, ProjectEntry entry)
                  in projects.indexed) ...<Widget>[
                if (index > 0) const HHairline(),
                _Line(
                  child: HCheckbox(
                    key: Key('start-cleanup-project-${entry.projectId}'),
                    label: entry.displayName,
                    hint: entry.path,
                    value: cleanup.projectIds.contains(entry.projectId),
                    onChanged: (bool _) =>
                        notifier.toggleProject(entry.projectId),
                  ),
                ),
              ],
              const HHairline(),
              _Line(
                child: HCheckbox(
                  key: const Key('start-cleanup-unassigned'),
                  label: l10n.startCleanupUnassigned,
                  value: cleanup.unassigned,
                  onChanged: (bool _) => notifier.toggleUnassigned(),
                ),
              ),
            ],
          ),
        ),
        SizedBox(height: tokens.spacing.x4),
        Padding(
          padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x4),
          child: HCheckbox(
            key: const Key('start-cleanup-waiting'),
            label: l10n.startCleanupWaiting,
            value: cleanup.includeWaitingChanges,
            onChanged: (bool _) => notifier.toggleWaitingChanges(),
          ),
        ),
        SizedBox(height: tokens.spacing.x5),
        Row(
          children: <Widget>[
            HButton(
              key: const Key('start-cleanup-review'),
              variant: HButtonVariant.primary,
              onPressed: cleanup.hasSelection && !cleanup.busy
                  ? () => unawaited(notifier.review())
                  : null,
              child: Text(l10n.startCleanupReview),
            ),
            if (result != null) ...<Widget>[
              SizedBox(width: tokens.spacing.x4),
              Expanded(
                child: Text(
                  l10n.startCleanupDone(
                    result.deleted,
                    formatBytes(result.bytesFreed),
                  ),
                  key: const Key('start-cleanup-done'),
                  style: tokens.typography.ui13.tinted(tokens.colors.fg1),
                ),
              ),
            ],
          ],
        ),
      ],
    );
  }
}

/// One line of the selection card.
class _Line extends StatelessWidget {
  const _Line({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.symmetric(
        horizontal: tokens.spacing.x4,
        vertical: tokens.spacing.x3,
      ),
      child: child,
    );
  }
}

/// The question before a cleanup: the plan of the daemon, item by item with
/// its size, its warnings, what stays and the fixed line of what is never
/// deleted. "Cancel" has the focus; only the danger button deletes, and it
/// names how many items and how many bytes (`docs/UX.md` 5.4).
class CleanupConfirm extends ConsumerWidget {
  /// Creates the question for [plan].
  const CleanupConfirm({required this.plan, super.key});

  /// The plan the daemon listed.
  final CleanupPlan plan;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final CleanupNotifier notifier = ref.read(cleanupProvider.notifier);
    final bool busy = ref.watch(
      cleanupProvider.select((CleanupState s) => s.busy),
    );
    final bool empty = plan.items.isEmpty;
    return HModal(
      key: const Key('start-cleanup-confirm'),
      title: Text(l10n.startCleanupConfirmTitle),
      onDismiss: busy ? null : notifier.cancel,
      scrimSemanticsLabel: l10n.startCleanupCancel,
      width: 620,
      actions: <Widget>[
        HButton(
          key: const Key('start-cleanup-cancel'),
          variant: HButtonVariant.secondary,
          autofocus: true,
          onPressed: busy ? null : notifier.cancel,
          child: Text(empty ? l10n.startCleanupClose : l10n.startCleanupCancel),
        ),
        if (!empty) ...<Widget>[
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('start-cleanup-delete'),
            variant: HButtonVariant.danger,
            onPressed: busy ? null : () => unawaited(notifier.confirm()),
            child: Text(
              l10n.startCleanupDelete(
                plan.items.length,
                formatBytes(plan.totalBytes),
              ),
            ),
          ),
        ],
      ],
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (empty)
            Text(
              l10n.startCleanupNothing,
              key: const Key('start-cleanup-nothing'),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            )
          else
            DecoratedBox(
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(tokens.radii.control),
                border: Border.all(color: tokens.colors.line),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  for (final (int index, CleanupItem item)
                      in plan.items.indexed) ...<Widget>[
                    if (index > 0) const HHairline(),
                    _PlanItem(index: index, item: item),
                  ],
                ],
              ),
            ),
          if (plan.kept.isNotEmpty) ...<Widget>[
            SizedBox(height: tokens.spacing.x4),
            Text(
              l10n.startCleanupKept,
              style: tokens.typography.ui11
                  .copyWith(fontWeight: FontWeight.w600)
                  .tinted(tokens.colors.fg2),
            ),
            SizedBox(height: tokens.spacing.x1),
            for (final String line in plan.kept)
              Padding(
                padding: EdgeInsets.only(top: tokens.spacing.x1 / 2),
                child: Text(
                  line,
                  style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                ),
              ),
          ],
          SizedBox(height: tokens.spacing.x4),
          DecoratedBox(
            decoration: BoxDecoration(
              color: tokens.colors.bg3,
              borderRadius: BorderRadius.circular(tokens.radii.control),
            ),
            child: Padding(
              padding: EdgeInsets.all(tokens.spacing.x3),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Padding(
                    padding: const EdgeInsets.only(top: 1),
                    child: HGlyphIcon(
                      HGlyph.lock,
                      size: 14,
                      color: tokens.colors.fg2,
                    ),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  Expanded(
                    child: Text(
                      plan.notDeleted,
                      key: const Key('start-cleanup-not-deleted'),
                      style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// One item of the plan: the daemon's sentence and the folder on the left,
/// the size on the right, the warning under both.
class _PlanItem extends StatelessWidget {
  const _PlanItem({required this.index, required this.item});

  final int index;
  final CleanupItem item;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      key: Key('start-cleanup-item-$index'),
      padding: EdgeInsets.symmetric(
        horizontal: tokens.spacing.x3,
        vertical: tokens.spacing.x2,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Expanded(
                child: Text(
                  item.description,
                  style: tokens.typography.ui13.tinted(tokens.colors.fg0),
                ),
              ),
              SizedBox(width: tokens.spacing.x3),
              Text(
                item.bytes > 0 ? formatBytes(item.bytes) : '–',
                style: tokens.typography.mono12.tinted(tokens.colors.fg1),
              ),
            ],
          ),
          if (item.warning.isNotEmpty) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            Text(
              item.warning,
              style: tokens.typography.ui12.tinted(tokens.colors.warningText),
            ),
          ],
        ],
      ),
    );
  }
}
