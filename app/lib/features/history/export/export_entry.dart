/// One flow as an export sees it: the recorded detail plus the bytes of its
/// bodies, already fetched through `GetBody`.
///
/// The encoders take this and nothing else. They touch no provider and no
/// widget, so they run on another isolate and a test can check the bytes they
/// produce without a widget tree.
library;

import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../../../core/domain/domain.dart';

/// A flow with its bodies.
@immutable
class HistoryExportEntry {
  /// Creates an entry.
  const HistoryExportEntry({
    required this.detail,
    this.requestBody,
    this.responseBody,
    this.originalBody,
  });

  /// Everything the daemon recorded about the flow.
  final FlowDetail detail;

  /// The recorded request body, or null when none was recorded.
  ///
  /// The body that went out: the edited one where there was an edit, the
  /// original otherwise. What each format does with it is the format's
  /// business; [originalRequestBody] and [editedRequestBody] keep the two
  /// apart for the ones that record both.
  final Uint8List? requestBody;

  /// The request body as it arrived, or null.
  Uint8List? get originalRequestBody =>
      detail.editedRequest == null ? requestBody : originalBody;

  /// The request body as the person changed it, or null.
  Uint8List? get editedRequestBody =>
      detail.editedRequest == null ? null : requestBody;

  /// The request body as it arrived, set when the caller fetched both.
  ///
  /// Read through [originalRequestBody] and [editedRequestBody]; those two
  /// know which of the pair [requestBody] is holding.
  final Uint8List? originalBody;

  /// The recorded response body, or null when none was recorded.
  final Uint8List? responseBody;

  /// The row of the flow.
  Flow get flow => detail.summary;
}

/// Bytes as a JSON value: text when they decode as UTF-8, base64 otherwise.
///
/// HAR 1.2 declares `text` a string, so binary content has to be base64 with
/// `encoding: "base64"` next to it; without the marker the file is invalid
/// UTF-8 and no viewer opens it (`backlog/sprint-2.md`, HUM-032, Fallstricke).
@immutable
class ExportedBytes {
  /// Creates a payload.
  const ExportedBytes({required this.text, required this.base64Encoded});

  /// Nothing was recorded.
  static const ExportedBytes empty = ExportedBytes(
    text: '',
    base64Encoded: false,
  );

  /// Decodes [bytes], preferring text.
  factory ExportedBytes.of(Uint8List? bytes) {
    if (bytes == null || bytes.isEmpty) {
      return empty;
    }
    try {
      return ExportedBytes(
        text: const Utf8Decoder().convert(bytes),
        base64Encoded: false,
      );
    } on FormatException {
      return ExportedBytes(text: base64.encode(bytes), base64Encoded: true);
    }
  }

  /// The content, as text or as base64.
  final String text;

  /// True when [text] is base64 of the original bytes.
  final bool base64Encoded;

  /// The value of the HAR `encoding` field, or null when the content is text.
  String? get encoding => base64Encoded ? 'base64' : null;
}
