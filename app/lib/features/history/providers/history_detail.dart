/// What the detail half of the history screen shows: which row is selected,
/// everything the daemon knows about it, and the recorded bodies.
///
/// The list holds summaries only; a body is fetched when a row is selected
/// and never before (`backlog/sprint-2.md`, HUM-032, Kontext).
library;

import 'dart:convert';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';

/// Above this many bytes a body is decoded on another isolate.
///
/// `docs/UX.md` 7: everything that touches a body runs in `Isolate.run` over
/// 64 KiB and returns plain Dart values. Below it the hop costs more than the
/// work.
const int historyIsolateThreshold = 64 * 1024;

/// How many lines of a body the view keeps. A recorded body can be eight
/// mebibytes; a person reads the first few thousand lines of it at most, and
/// the rest is what the export is for.
const int historyBodyMaxLines = 20000;

/// Which flow the detail half shows, or null.
final NotifierProvider<HistorySelectionNotifier, FlowId?>
historySelectionProvider = NotifierProvider<HistorySelectionNotifier, FlowId?>(
  HistorySelectionNotifier.new,
);

/// The notifier behind [historySelectionProvider].
class HistorySelectionNotifier extends Notifier<FlowId?> {
  @override
  FlowId? build() => null;

  /// Selects [id].
  void select(FlowId id) => state = id;

  /// Selects nothing.
  void clear() => state = null;
}

/// Everything the daemon knows about one recorded flow.
final historyDetailProvider = FutureProvider.autoDispose
    .family<FlowDetail, FlowId>(
      (Ref ref, FlowId id) => ref.watch(daemonClientProvider).getFlow(id),
    );

/// A recorded body, decoded for reading.
///
/// The canonical name of `backlog/CONVENTIONS.md` 3.9 for this is
/// `flowBodyProvider(BodyRef)` in `core`; it lives here until a second screen
/// needs it, so that this issue adds nothing to a file two other screens are
/// being built in.
final historyBodyProvider = FutureProvider.autoDispose
    .family<HistoryBody, BodyRef>((Ref ref, BodyRef reference) async {
      if (reference.isEmpty) {
        return HistoryBody.empty;
      }
      final DaemonClient client = ref.watch(daemonClientProvider);
      final BytesBuilder buffer = BytesBuilder(copy: false);
      await for (final Uint8List chunk in client.getBody(reference)) {
        buffer.add(chunk);
      }
      final Uint8List bytes = buffer.takeBytes();
      if (bytes.length > historyIsolateThreshold) {
        return Isolate.run(() => decodeHistoryBody(bytes, reference.truncated));
      }
      return decodeHistoryBody(bytes, reference.truncated);
    });

/// A recorded body as the view reads it: lines of text, or the note that it
/// is not text at all.
@immutable
class HistoryBody {
  /// Creates a decoded body.
  const HistoryBody({
    required this.lines,
    required this.byteCount,
    required this.binary,
    required this.truncated,
  });

  /// Nothing was recorded.
  static const HistoryBody empty = HistoryBody(
    lines: <String>[],
    byteCount: 0,
    binary: false,
    truncated: false,
  );

  /// The lines, without their terminators, at most [historyBodyMaxLines].
  final List<String> lines;

  /// How many bytes the recorded body has.
  final int byteCount;

  /// True when the bytes are not text and the view offers a hex dump.
  final bool binary;

  /// True when the recorder stopped before the end of the body.
  final bool truncated;

  /// True when there is nothing to show.
  bool get isEmpty => byteCount == 0;

  /// True when this view stopped drawing before the end of the body.
  ///
  /// Not the same as [truncated]: that one says the *recorder* stopped, this
  /// one says the view did. The sentences differ, and so do the remedies —
  /// what the view cut, the export still carries.
  bool get linesCapped => lines.length >= historyBodyMaxLines;

  /// True when [lines] stops before the end of the body, for either reason.
  bool get linesTruncated => truncated || linesCapped;

  @override
  bool operator ==(Object other) =>
      other is HistoryBody &&
      listEquals(other.lines, lines) &&
      other.byteCount == byteCount &&
      other.binary == binary &&
      other.truncated == truncated;

  @override
  int get hashCode =>
      Object.hash(Object.hashAll(lines), byteCount, binary, truncated);
}

/// Turns recorded [bytes] into lines.
///
/// A `NUL` byte in the first kibibyte is the test for "not text": it is what
/// every editor uses, it costs one pass, and it never calls a UTF-8 file
/// binary. Invalid sequences are replaced rather than thrown, because a body
/// that fails to decode must still be readable up to the point where it
/// broke. Top level and free of any Flutter type so that `Isolate.run` can
/// send the result back (`docs/UX.md` 7).
HistoryBody decodeHistoryBody(Uint8List bytes, bool truncated) {
  final int probe = bytes.length < 1024 ? bytes.length : 1024;
  for (int i = 0; i < probe; i++) {
    if (bytes[i] == 0) {
      return HistoryBody(
        lines: const <String>[],
        byteCount: bytes.length,
        binary: true,
        truncated: truncated,
      );
    }
  }
  final String text = const Utf8Decoder(allowMalformed: true).convert(bytes);
  final List<String> lines = const LineSplitter().convert(text);
  return HistoryBody(
    lines: lines.length > historyBodyMaxLines
        ? lines.sublist(0, historyBodyMaxLines)
        : lines,
    byteCount: bytes.length,
    binary: false,
    truncated: truncated,
  );
}
