/// The seam between the history and a file on disk.
///
/// The encoders are pure and tested; this file is what turns their bytes into
/// a file somebody can open. `file_picker` 12 writes the file itself and
/// hands back the `Uri` it wrote to, so the seam is one call and not the
/// two-step "ask for a path, then write it" the sprint sketch assumed
/// (`backlog/sprint-2.md`, HUM-032, Fallstricke: that pitfall belongs to the
/// older API).
library;

import 'dart:convert';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';

import 'csv.dart';
import 'curl.dart';
import 'export_entry.dart';
import 'har.dart';
import 'jsonl.dart';

/// The largest number of flows one export covers.
///
/// Beyond it the menu says so instead of starting a job that would hold every
/// body of five thousand flows in memory at once.
const int historyExportMaxFlows = 5000;

/// What an export writes.
enum HistoryExportFormat {
  /// HAR 1.2, the format every browser's network panel reads.
  har,

  /// JSON Lines, one flow per line, bodies base64.
  jsonl,

  /// The columns of the table, for a spreadsheet.
  csv,

  /// A single flow as a `curl` command line.
  curl;

  /// The media type of the file this format writes.
  String get mimeType => switch (this) {
    HistoryExportFormat.har => 'application/json',
    HistoryExportFormat.jsonl => 'application/x-ndjson',
    HistoryExportFormat.csv => 'text/csv',
    HistoryExportFormat.curl => 'text/x-shellscript',
  };

  /// The extension of the file this format writes, without the dot.
  String get fileExtension => switch (this) {
    HistoryExportFormat.har => 'har',
    HistoryExportFormat.jsonl => 'jsonl',
    HistoryExportFormat.csv => 'csv',
    HistoryExportFormat.curl => 'sh',
  };

  /// True for the format that covers exactly one flow.
  bool get singleFlowOnly => this == HistoryExportFormat.curl;
}

/// The bytes of one export, plus the extra file `curl` needs.
@immutable
class HistoryExportResult {
  /// Creates a result.
  const HistoryExportResult({
    required this.format,
    required this.bytes,
    required this.flowCount,
    this.bodyFile,
  });

  /// Which format was written.
  final HistoryExportFormat format;

  /// The document itself, UTF-8.
  final Uint8List bytes;

  /// How many flows it covers.
  final int flowCount;

  /// The request body that belongs next to a `curl` command, or null.
  final Uint8List? bodyFile;
}

/// Encodes [entries] in [format].
///
/// Top level and free of Flutter types on purpose: `Isolate.run` can call it
/// and hand the bytes back (`docs/UX.md` 7).
HistoryExportResult encodeHistoryExport({
  required HistoryExportFormat format,
  required List<HistoryExportEntry> entries,
  required String creatorVersion,
}) {
  if (format.singleFlowOnly && entries.length != 1) {
    throw ArgumentError.value(
      entries.length,
      'entries',
      'the ${format.name} export covers exactly one flow',
    );
  }
  final String text = switch (format) {
    HistoryExportFormat.har => encodeHar(
      entries: entries,
      creatorVersion: creatorVersion,
    ),
    HistoryExportFormat.jsonl => encodeJsonl(entries),
    HistoryExportFormat.csv => encodeCsv(entries),
    HistoryExportFormat.curl => encodeCurl(entries.single),
  };
  return HistoryExportResult(
    format: format,
    bytes: Uint8List.fromList(utf8.encode(text)),
    flowCount: entries.length,
    bodyFile: format == HistoryExportFormat.curl
        ? entries.single.requestBody
        : null,
  );
}

/// What an export covers.
enum HistoryExportScope {
  /// The one row that is selected.
  selected,

  /// Everything the current filter matches, up to [historyExportMaxFlows].
  filtered,
}

/// Where an export ends up.
///
/// An interface with one implementation is usually a defect; this one is the
/// named seam of the issue, and the second implementation is the one every
/// test uses, because a widget test must not open a file dialog.
abstract class HistoryExportTarget {
  /// Asks where to write and writes [result] there.
  ///
  /// Returns every file that was created, or an empty list when the person
  /// dismissed the dialog. [fileName] is the name the dialog offers.
  Future<List<String>> save(
    HistoryExportResult result, {
    required String fileName,
    required String dialogTitle,
  });
}

/// The first free name of [wanted] in [directory].
///
/// `request.body`, then `request-2.body`, and so on. The dialog asked about
/// one file; the second one is the export's own idea, so it may not take a
/// name that is already in use.
File _freeName(String directory, String wanted) {
  final int dot = wanted.lastIndexOf('.');
  final String stem = dot <= 0 ? wanted : wanted.substring(0, dot);
  final String suffix = dot <= 0 ? '' : wanted.substring(dot);
  for (int n = 1; n < 1000; n++) {
    final String name = n == 1 ? wanted : '$stem-$n$suffix';
    final File candidate = File('$directory${Platform.pathSeparator}$name');
    if (!candidate.existsSync()) {
      return candidate;
    }
  }
  throw const FileSystemException(
    'no free name for the body file beside the command',
  );
}

/// The save dialog of the desktop.
///
/// On Linux `file_picker` speaks to the XDG desktop portal over D-Bus, so the
/// dialog is the one the desktop uses everywhere else; nothing is drawn by
/// this application.
class FilePickerExportTarget implements HistoryExportTarget {
  /// Creates the target.
  const FilePickerExportTarget();

  @override
  Future<List<String>> save(
    HistoryExportResult result, {
    required String fileName,
    required String dialogTitle,
  }) async {
    final Uri? written = await FilePicker.saveFile(
      fileName: fileName,
      bytes: result.bytes,
      mimeType: result.format.mimeType,
      dialogTitle: dialogTitle,
    );
    if (written == null) {
      // Dismissed. Nothing was written, and the surface says exactly that.
      return const <String>[];
    }
    return writeBeside(
      result,
      written.isScheme('file') ? written.toFilePath() : written.toString(),
    );
  }

  /// Writes the file the dialog named, plus the body file a `curl` export
  /// needs beside it.
  ///
  /// Split out so that a test can exercise the part that does not open a
  /// dialog: what happens to a file that is already there is the half worth
  /// proving.
  @visibleForTesting
  Future<List<String>> writeBeside(
    HistoryExportResult result,
    String path,
  ) async {
    final List<String> paths = <String>[path];
    // The dialog wrote the chosen file itself; only the second one is ours.
    final Uint8List? body = result.bodyFile;
    if (body != null && body.isNotEmpty) {
      // The command reads the body from a file beside it. The name is said
      // in the modal before the export runs, and an existing file is never
      // overwritten: nobody asked for that file, so nothing of theirs may
      // disappear under it.
      final File beside = _freeName(File(path).parent.path, curlBodyFileName);
      await beside.writeAsBytes(body, flush: true);
      paths.add(beside.path);
    }
    return paths;
  }
}
