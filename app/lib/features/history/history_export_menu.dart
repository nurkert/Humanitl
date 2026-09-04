/// The export control and the modal behind it.
///
/// The modal asks two questions — what, and in which format — says the cap
/// before the job starts, shows progress in rows while it runs, and names
/// every file it wrote. It is a modal and not a sheet because the answer is
/// needed before anything happens; `HModal` brings its own focus scope and
/// `Escape`.
library;

import 'dart:async';

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'export/curl.dart';
import 'export/history_export.dart';
import 'history_filter_bar.dart';
import 'history_view.dart';
import 'providers/history_detail.dart';
import 'providers/history_export.dart';
import 'providers/history_page.dart';

/// The button that opens the export modal.
class HistoryExportButton extends ConsumerWidget {
  /// Creates the button.
  const HistoryExportButton({required this.onOpen, super.key});

  /// Opens the modal.
  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    // Ghost, like every other control on this screen: the accent belongs to
    // the filter field, and the history takes no decision (`docs/UX.md` 3.1).
    return HButton(
      key: const Key('history-export-open'),
      variant: HButtonVariant.ghost,
      onPressed: onOpen,
      child: Text(l10n.historyExport),
    );
  }
}

/// The export modal.
class HistoryExportModal extends ConsumerStatefulWidget {
  /// Creates the modal.
  const HistoryExportModal({required this.onClose, super.key});

  /// Closes the modal.
  final VoidCallback onClose;

  @override
  ConsumerState<HistoryExportModal> createState() => _HistoryExportModalState();
}

class _HistoryExportModalState extends ConsumerState<HistoryExportModal> {
  HistoryExportFormat _format = HistoryExportFormat.har;
  HistoryExportScope? _scope;

  /// The file name the save dialog offers: what it holds and when it was
  /// taken, so two exports of one session do not overwrite each other.
  String _fileName(HistoryExportFormat format) {
    final String stamp = formatHistoryTimestamp(
      DateTime.now(),
    ).replaceAll(RegExp('[^0-9]'), '');
    return 'humanitl-$stamp.${format.fileExtension}';
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final HistoryExportState job = ref.watch(historyExportProvider);
    final HistoryPageState page = ref.watch(historyPageProvider);
    final bool hasSelection = ref.watch(historySelectionProvider) != null;
    final HistoryExportScope scope =
        _scope ??
        (hasSelection
            ? HistoryExportScope.selected
            : HistoryExportScope.filtered);
    final bool overCap = page.capped || page.total > historyExportMaxFlows;
    final String matched = page.capped
        ? l10n.historyTotalAtLeast(page.total)
        : l10n.historyTotalExact(page.total);

    return HModal(
      title: Text(l10n.historyExportTitle),
      onDismiss: job.running ? null : _close,
      // Wide enough for four formats side by side; a segmented control that
      // overflows would hide the one somebody is looking for.
      width: 600,
      actions: <Widget>[
        if (!job.running)
          HButton(
            variant: HButtonVariant.ghost,
            onPressed: _close,
            child: Text(
              job.phase == HistoryExportPhase.done
                  ? l10n.historyExportClose
                  : l10n.historyExportCancel,
            ),
          ),
        if (job.phase != HistoryExportPhase.done)
          HButton(
            key: const Key('history-export-save'),
            variant: HButtonVariant.primary,
            onPressed: job.running
                ? null
                : () => unawaited(
                    ref
                        .read(historyExportProvider.notifier)
                        .run(
                          format: _format,
                          scope: scope,
                          fileName: _fileName(_format),
                          dialogTitle: l10n.historyExportDialogTitle,
                        ),
                  ),
            child: Text(l10n.historyExportSave),
          ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            l10n.historyExportScope,
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
          SizedBox(height: tokens.spacing.x1),
          HSegmented<HistoryExportScope>(
            options: <HSegmentOption<HistoryExportScope>>[
              if (hasSelection)
                HSegmentOption<HistoryExportScope>(
                  value: HistoryExportScope.selected,
                  label: l10n.historyExportScopeSelected,
                ),
              HSegmentOption<HistoryExportScope>(
                value: HistoryExportScope.filtered,
                label: l10n.historyExportScopeFiltered(matched),
              ),
            ],
            selected: scope,
            enabled: !job.running,
            onSelect: (HistoryExportScope value) =>
                setState(() => _scope = value),
          ),
          SizedBox(height: tokens.spacing.x3),
          Text(
            l10n.historyExportFormat,
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
          SizedBox(height: tokens.spacing.x1),
          HSegmented<HistoryExportFormat>(
            options: <HSegmentOption<HistoryExportFormat>>[
              for (final HistoryExportFormat format
                  in HistoryExportFormat.values)
                if (!format.singleFlowOnly ||
                    scope == HistoryExportScope.selected)
                  HSegmentOption<HistoryExportFormat>(
                    value: format,
                    label: format.name.toUpperCase(),
                  ),
            ],
            selected:
                _format.singleFlowOnly && scope != HistoryExportScope.selected
                ? HistoryExportFormat.har
                : _format,
            enabled: !job.running,
            onSelect: (HistoryExportFormat value) =>
                setState(() => _format = value),
          ),
          if (scope == HistoryExportScope.filtered && overCap) ...<Widget>[
            SizedBox(height: tokens.spacing.x3),
            Text(
              l10n.historyExportCap(historyExportMaxFlows),
              style: tokens.typography.ui13.tinted(tokens.colors.fg0),
            ),
          ],
          SizedBox(height: tokens.spacing.x3),
          // Before anything is written, not after: whoever passes the file on
          // has to know what is in it. There is no redaction yet
          // (`docs/SECURITY.md`, Aufzeichnung).
          Text(
            key: const Key('history-export-contents'),
            l10n.historyExportContents,
            style: tokens.typography.ui13.tinted(
              tokens.stateTextColor(HFlowState.held),
            ),
          ),
          // The curl format writes a second file beside the chosen one. An
          // irreversible action says so beforehand (`docs/UX.md` 5.4).
          if (_format == HistoryExportFormat.curl) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            Text(
              key: const Key('history-export-beside'),
              l10n.historyExportBesideFile(curlBodyFileName),
              style: tokens.typography.ui13.tinted(
                tokens.stateTextColor(HFlowState.held),
              ),
            ),
          ],
          SizedBox(height: tokens.spacing.x3),
          _Progress(job: job),
        ],
      ),
    );
  }

  void _close() {
    ref.read(historyExportProvider.notifier).reset();
    widget.onClose();
  }
}

/// What the job is doing, in rows and in full paths.
class _Progress extends StatelessWidget {
  const _Progress({required this.job});

  final HistoryExportState job;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Diagnostic? failure = job.failure;
    return switch (job.phase) {
      HistoryExportPhase.idle => const SizedBox.shrink(),
      HistoryExportPhase.collecting => Text(
        l10n.historyExportCollecting(
          l10n.historyTotalExact(job.done),
          l10n.historyTotalExact(job.total),
        ),
        style: tokens.typography.ui13.tinted(tokens.colors.fg1),
      ),
      HistoryExportPhase.writing => Text(
        l10n.historyExportWriting(l10n.historyTotalExact(job.total)),
        style: tokens.typography.ui13.tinted(tokens.colors.fg1),
      ),
      HistoryExportPhase.done => Text(
        // The full path, in monospace, because it is the thing somebody has
        // to find again (`backlog/CONVENTIONS.md` 4.13).
        l10n.historyExportWrote(job.written.join(' · ')),
        style: tokens.typography.mono12.tinted(tokens.colors.fg0),
      ),
      HistoryExportPhase.cancelled => Text(
        l10n.historyExportCancelled,
        style: tokens.typography.ui13.tinted(tokens.colors.fg1),
      ),
      HistoryExportPhase.empty => Text(
        l10n.historyExportNothing,
        style: tokens.typography.ui13.tinted(tokens.colors.fg1),
      ),
      HistoryExportPhase.failed =>
        failure == null
            ? const SizedBox.shrink()
            // A failure of the daemon has a registered code and gets the
            // card; one of ours has none, and a card with an empty code chip
            // would claim a register entry that does not exist.
            : failure.code.isEmpty
            ? Text(
                '${l10n.historyExportFailedTitle} · ${failure.why}',
                style: tokens.typography.ui13.tinted(
                  tokens.stateTextColor(HFlowState.error),
                ),
              )
            : HDiagnosticCard(
                code: failure.code,
                severityLabel: historySeverityLabel(l10n, failure.severity),
                color: historySeverityColor(tokens, failure.severity),
                title: l10n.historyExportFailedTitle,
                why: failure.why,
                docsUrl: failure.docsUrl,
                width: double.infinity,
              ),
    };
  }
}
