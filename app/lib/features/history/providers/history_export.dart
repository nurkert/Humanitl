/// Running an export: gather the flows, fetch their bodies, encode, save.
///
/// The job is a state machine with a visible count, because the specification
/// asks the menu to show progress in rows ("1,284 of 5,000") rather than a
/// spinner — the number is known, and a spinner would say less than the number
/// does (`docs/UX.md` 2.11).
library;

import 'dart:async';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../export/export_entry.dart';
import '../export/history_export.dart';
import 'history_detail.dart';
import 'history_page.dart';
import 'history_query.dart';

/// Which version the export writes into its creator block.
///
/// The daemon's version belongs to the daemon; what wrote the file is this
/// application, and until it carries a version of its own the constant says
/// so rather than borrowing one.
const String historyExportCreatorVersion = '0.0.0';

/// Where an export ends up. Overridden in tests, which write nowhere.
final Provider<HistoryExportTarget> historyExportTargetProvider =
    Provider<HistoryExportTarget>((Ref ref) => const FilePickerExportTarget());

/// How an export turns entries into bytes.
typedef HistoryExportEncoder =
    Future<HistoryExportResult> Function({
      required HistoryExportFormat format,
      required List<HistoryExportEntry> entries,
      required String creatorVersion,
    });

/// Encodes on another isolate.
///
/// A five thousand flow HAR is seconds of string building, and it does not
/// happen on the thread that draws (`docs/UX.md` 7). Top level, so the
/// closure carries nothing but its arguments.
Future<HistoryExportResult> isolateHistoryExportEncoder({
  required HistoryExportFormat format,
  required List<HistoryExportEntry> entries,
  required String creatorVersion,
}) => Isolate.run(
  () => encodeHistoryExport(
    format: format,
    entries: entries,
    creatorVersion: creatorVersion,
  ),
);

/// The encoder in use. A widget test overrides it with the synchronous one:
/// `Isolate.run` needs real time, and a widget test runs on a fake clock.
final Provider<HistoryExportEncoder> historyExportEncoderProvider =
    Provider<HistoryExportEncoder>((Ref ref) => isolateHistoryExportEncoder);

/// What an export job is doing right now.
enum HistoryExportPhase {
  /// Nothing is running.
  idle,

  /// The flows are being read from the daemon.
  collecting,

  /// The bytes are being encoded and written.
  writing,

  /// A file was written.
  done,

  /// The person dismissed the save dialog.
  cancelled,

  /// The set to export turned out to be empty.
  empty,

  /// Something failed; see [HistoryExportState.failure].
  failed,
}

/// The state of the export job.
@immutable
class HistoryExportState {
  /// Creates a state.
  const HistoryExportState({
    this.phase = HistoryExportPhase.idle,
    this.done = 0,
    this.total = 0,
    this.written = const <String>[],
    this.failure,
  });

  /// Nothing has run yet.
  static const HistoryExportState idle = HistoryExportState();

  /// What the job is doing.
  final HistoryExportPhase phase;

  /// How many flows are gathered so far.
  final int done;

  /// How many flows the job covers in total.
  final int total;

  /// The files that were written.
  final List<String> written;

  /// Why it failed, or null.
  final Diagnostic? failure;

  /// True while the job holds the menu.
  bool get running =>
      phase == HistoryExportPhase.collecting ||
      phase == HistoryExportPhase.writing;

  @override
  bool operator ==(Object other) =>
      other is HistoryExportState &&
      other.phase == phase &&
      other.done == done &&
      other.total == total &&
      listEquals(other.written, written) &&
      other.failure == failure;

  @override
  int get hashCode =>
      Object.hash(phase, done, total, Object.hashAll(written), failure);
}

/// The export job.
final NotifierProvider<HistoryExportNotifier, HistoryExportState>
historyExportProvider =
    NotifierProvider<HistoryExportNotifier, HistoryExportState>(
      HistoryExportNotifier.new,
    );

/// The notifier behind [historyExportProvider].
class HistoryExportNotifier extends Notifier<HistoryExportState> {
  @override
  HistoryExportState build() => HistoryExportState.idle;

  /// Forgets the last result, so the menu opens clean.
  void reset() => state = HistoryExportState.idle;

  /// Runs one export and saves it.
  ///
  /// [scope] decides what is covered: the selected row, or everything the
  /// current filter matches up to [historyExportMaxFlows]. The cap is not a
  /// silent truncation — the menu says the number before the job starts.
  Future<void> run({
    required HistoryExportFormat format,
    required HistoryExportScope scope,
    required String fileName,
    required String dialogTitle,
  }) async {
    if (state.running) {
      return;
    }
    state = const HistoryExportState(phase: HistoryExportPhase.collecting);
    try {
      final List<Flow> flows = await _flows(scope);
      if (flows.isEmpty) {
        // Nothing matched. No dialog was ever opened, so nothing was
        // "closed" either.
        state = const HistoryExportState(phase: HistoryExportPhase.empty);
        return;
      }
      state = HistoryExportState(
        phase: HistoryExportPhase.collecting,
        total: flows.length,
      );
      final List<HistoryExportEntry> entries = <HistoryExportEntry>[];
      final DaemonClient client = ref.read(daemonClientProvider);
      for (final Flow flow in flows) {
        entries.add(await _entry(client, flow));
        state = HistoryExportState(
          phase: HistoryExportPhase.collecting,
          done: entries.length,
          total: flows.length,
        );
      }
      state = HistoryExportState(
        phase: HistoryExportPhase.writing,
        done: entries.length,
        total: flows.length,
      );
      final HistoryExportResult result =
          await ref.read(historyExportEncoderProvider)(
            format: format,
            entries: entries,
            creatorVersion: historyExportCreatorVersion,
          );
      final List<String> written = await ref
          .read(historyExportTargetProvider)
          .save(result, fileName: fileName, dialogTitle: dialogTitle);
      if (!ref.mounted) {
        return;
      }
      state = HistoryExportState(
        phase: written.isEmpty
            ? HistoryExportPhase.cancelled
            : HistoryExportPhase.done,
        done: entries.length,
        total: flows.length,
        written: written,
      );
    } on DaemonException catch (error) {
      if (ref.mounted) {
        state = HistoryExportState(
          phase: HistoryExportPhase.failed,
          failure: error.diagnostic,
        );
      }
    } on Object catch (error) {
      if (ref.mounted) {
        state = HistoryExportState(
          phase: HistoryExportPhase.failed,
          // No code: the register has none for "the export could not be
          // written", and the app never invents one
          // (`backlog/CONVENTIONS.md` 4.6). `UI_002` exists but means the
          // tray icon has no room. Until there is a code, the failure is
          // shown as the sentence it is; the modal reads an empty code and
          // leaves the card out.
          failure: Diagnostic(
            code: '',
            severity: Severity.error,
            why: '$error',
          ),
        );
      }
    }
  }

  /// The flows [scope] covers, newest first, at most
  /// [historyExportMaxFlows].
  Future<List<Flow>> _flows(HistoryExportScope scope) async {
    // The number to count against is what the filter matches, capped by what
    // one export carries -- known before the first page comes back, so the
    // progress reads "1,284 of 5,000" and not "200 of 200"
    // (`docs/UX.md` 2.11).
    if (scope == HistoryExportScope.selected) {
      final FlowId? id = ref.read(historySelectionProvider);
      if (id == null) {
        return const <Flow>[];
      }
      final Flow? row = ref
          .read(historyPageProvider)
          .rows
          .where((Flow flow) => flow.id == id)
          .firstOrNull;
      return row == null ? const <Flow>[] : <Flow>[row];
    }
    // The list on screen holds at most `historyMaxRows`; an export may cover
    // more, so it pages the daemon itself rather than exporting the window.
    final HistoryQuery query = ref.read(historyQueryProvider);
    final DaemonClient client = ref.read(daemonClientProvider);
    final List<Flow> flows = <Flow>[];
    final Set<String> seen = <String>{};
    String? cursor;
    int expected = 0;
    while (flows.length < historyExportMaxFlows) {
      final FlowPage page = await client.listFlows(
        query.flowFilter,
        limit: historyPageSize,
        cursor: cursor,
      );
      if (expected == 0) {
        expected = page.total > historyExportMaxFlows
            ? historyExportMaxFlows
            : page.total;
      }
      final int before = flows.length;
      for (final Flow flow in page.flows) {
        if (seen.add(flow.id.value) && flows.length < historyExportMaxFlows) {
          flows.add(flow);
        }
      }
      // A page that adds no row cannot be followed by one that does; a
      // daemon that names a fresh cursor every time would otherwise turn
      // for ever.
      if (flows.length == before) {
        break;
      }
      state = HistoryExportState(
        phase: HistoryExportPhase.collecting,
        done: flows.length,
        total: expected < flows.length ? flows.length : expected,
      );
      // A cursor that does not move is a daemon that cannot go on. Stopping
      // only at an empty one would loop forever without the list growing.
      if (page.nextCursor.isEmpty || page.nextCursor == cursor) {
        break;
      }
      cursor = page.nextCursor;
    }
    return flows;
  }

  /// One flow with its recorded bodies.
  Future<HistoryExportEntry> _entry(DaemonClient client, Flow flow) async {
    final FlowDetail detail = await client.getFlow(flow.id);
    return HistoryExportEntry(
      detail: detail,
      requestBody: await _body(
        client,
        detail.editedRequest?.body ?? detail.request?.body,
      ),
      // Where there was an edit, both bodies are fetched: the record has
      // two, and a format that keeps both must be able to write both.
      originalBody: detail.editedRequest == null
          ? null
          : await _body(client, detail.request?.body),
      responseBody: await _body(client, detail.responseBody),
    );
  }

  Future<Uint8List?> _body(DaemonClient client, BodyRef? reference) async {
    if (reference == null || reference.isEmpty) {
      return null;
    }
    final BytesBuilder buffer = BytesBuilder(copy: false);
    await for (final Uint8List chunk in client.getBody(reference)) {
      buffer.add(chunk);
    }
    return buffer.takeBytes();
  }
}
