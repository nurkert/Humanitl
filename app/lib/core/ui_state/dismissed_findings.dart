/// Which standing findings the person has closed (HUM-302).
///
/// A finding such as `LLM_006` ("the LLM endpoint is not on a private
/// network") is not an event: the daemon puts it into every answer about the
/// sandbox for as long as the endpoint stays where it is. A notice closed for
/// the answer on screen would come back with the next one. The dismissal is
/// therefore kept for the finding, not for the card: the code plus a
/// fingerprint of what it is about.
///
/// # What the fingerprint covers
///
/// The code and the cause, the sentence the daemon wrote. The cause names
/// the subject (the endpoint of `LLM_006`, the path of a refused mount), so a
/// finding about another subject has another fingerprint and shows again,
/// while the same finding about the same subject stays closed. The
/// fingerprint is FNV-1a over UTF-8, 32 bits, in hexadecimal: an identifier
/// for equality, not a secret and not a protection. The cause itself is not
/// written to the file, only this number.
///
/// # Where it is kept
///
/// In the UI state file of HUM-254 (`ui-state.json`), one list per code under
/// [dismissedFindingKey], at most [dismissedFindingsPerCode] entries, the
/// newest last. A write that does not go through leaves the dismissal in
/// memory for this run, as everywhere in that file.
library;

import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../domain/domain.dart';
import 'ui_state_file.dart';

/// The common start of every key this store writes.
const String dismissedFindingPrefix = 'findingDismissed.';

/// How many subjects per code are remembered. Older ones fall out first.
const int dismissedFindingsPerCode = 8;

/// The key under which the dismissed fingerprints of [code] stand.
String dismissedFindingKey(String code) => '$dismissedFindingPrefix$code';

/// The fingerprint of [diagnostic]: code and cause, nothing else.
String findingFingerprint(Diagnostic diagnostic) {
  int hash = 0x811c9dc5;
  for (final int byte in utf8.encode('${diagnostic.code}\n${diagnostic.why}')) {
    hash ^= byte;
    hash = (hash * 0x01000193) & 0xffffffff;
  }
  return hash.toRadixString(16).padLeft(8, '0');
}

/// The dismissed fingerprints, per code.
final NotifierProvider<DismissedFindings, Map<String, List<String>>>
dismissedFindingsProvider =
    NotifierProvider<DismissedFindings, Map<String, List<String>>>(
      DismissedFindings.new,
    );

/// The store.
///
/// Read once, synchronously, at the first build, for the reason the rail
/// gives: asynchronously the notice would flash for a frame first.
class DismissedFindings extends Notifier<Map<String, List<String>>> {
  @override
  Map<String, List<String>> build() => <String, List<String>>{
    for (final MapEntry<String, Object?> entry
        in ref.read(uiStateFileProvider).read().entries)
      if (entry.key.startsWith(dismissedFindingPrefix) && entry.value is List)
        entry.key.substring(dismissedFindingPrefix.length): <String>[
          for (final Object? item in entry.value! as List<Object?>)
            if (item is String) item,
        ],
  };

  /// True when [diagnostic], about this very subject, was closed before.
  bool covers(Diagnostic diagnostic) =>
      state[diagnostic.code]?.contains(findingFingerprint(diagnostic)) ?? false;

  /// Remembers that [diagnostic] was closed, and writes it down.
  void dismiss(Diagnostic diagnostic) {
    final String fingerprint = findingFingerprint(diagnostic);
    final List<String> before = state[diagnostic.code] ?? const <String>[];
    if (before.contains(fingerprint)) {
      return;
    }
    final List<String> after = <String>[...before, fingerprint];
    final List<String> kept = after.length > dismissedFindingsPerCode
        ? after.sublist(after.length - dismissedFindingsPerCode)
        : after;
    state = <String, List<String>>{...state, diagnostic.code: kept};
    ref
        .read(uiStateFileProvider)
        .setValue(dismissedFindingKey(diagnostic.code), kept);
  }
}
