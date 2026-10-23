/// What a clipboard leaves on a pasted endpoint or access key (HUM-283).
///
/// Copied out of a configuration file, an address arrives as
/// `https://llm.example/api/v1/"` and a key as `"sk-…"`. The daemon refuses
/// the first (`CONFIG_003`) and strips the second, but the field should not
/// show the debris in the first place: on paste and on submit, white space
/// around the text and the quotes of a clipboard come off. Nothing inside the
/// text is touched; what is left there is the daemon's to judge.
///
/// The same rule as `humanitl_config::pasted::strip_paste_debris` in the
/// daemon, so app and daemon agree on what a clean value is.
library;

import 'package:flutter/services.dart';

const String _quotes = '"\'';

/// [text] without surrounding white space and without the quotes of a
/// clipboard.
///
/// A pair goes only when both ends hold the same quote (`"value"`,
/// `'value'`), together with white space inside it (`" value "`). Otherwise a
/// single quote goes at one edge only, the leading one first (`"value` and
/// `value"`); `"value'` keeps its `'`. `String.trim` also removes U+FEFF, as
/// the daemon does.
String stripPasteDebris(String text) {
  final String rest = text.trim();
  final bool quotedStart = rest.isNotEmpty && _quotes.contains(rest[0]);
  final bool quotedEnd =
      rest.isNotEmpty && _quotes.contains(rest[rest.length - 1]);
  final String inner;
  if (rest.length >= 2 && quotedStart && rest[0] == rest[rest.length - 1]) {
    inner = rest.substring(1, rest.length - 1);
  } else if (quotedStart) {
    inner = rest.substring(1);
  } else if (quotedEnd) {
    inner = rest.substring(0, rest.length - 1);
  } else {
    inner = rest;
  }
  return inner.trim();
}

/// Cleans the inserted text of a paste with [stripPasteDebris], and leaves
/// typing alone.
///
/// The edit is located by what changed between the two values: the common
/// head and tail stay, the part in between is what came in, whether it was
/// inserted at the cursor or replaced a selection. A paste is an insertion of
/// more than one character. Only that inserted part is cleaned; text that was
/// already in the field is never touched, and a character typed into the
/// middle of an address stays until the field is submitted.
class PasteDebrisFormatter extends TextInputFormatter {
  /// Creates the formatter.
  const PasteDebrisFormatter();

  @override
  TextEditingValue formatEditUpdate(
    TextEditingValue oldValue,
    TextEditingValue newValue,
  ) {
    final String after = newValue.text;
    final (int head, int tail) = _editRange(oldValue, after);
    final String inserted = after.substring(head, after.length - tail);
    if (inserted.length < 2) {
      return newValue;
    }
    final String clean = stripPasteDebris(inserted);
    if (clean == inserted) {
      return newValue;
    }
    final String text =
        after.substring(0, head) + clean + after.substring(after.length - tail);
    return TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: head + clean.length),
    );
  }

  /// Where the edit sits in [after]: the length of the unchanged head and of
  /// the unchanged tail.
  ///
  /// The old selection decides first: whatever stood in it (the cursor, or
  /// a selected range a paste replaces) was replaced by the new text in
  /// between. Only when the old selection does not explain the new text
  /// does the common head and tail of both texts stand in.
  static (int, int) _editRange(TextEditingValue oldValue, String after) {
    final String before = oldValue.text;
    final TextSelection selection = oldValue.selection;
    if (selection.isValid &&
        selection.start >= 0 &&
        selection.end <= before.length) {
      final String head = before.substring(0, selection.start);
      final String tail = before.substring(selection.end);
      if (after.length >= head.length + tail.length &&
          after.startsWith(head) &&
          after.endsWith(tail)) {
        return (head.length, tail.length);
      }
    }
    int head = 0;
    final int shorter = before.length < after.length
        ? before.length
        : after.length;
    while (head < shorter && before[head] == after[head]) {
      head++;
    }
    int tail = 0;
    while (tail < shorter - head &&
        before[before.length - 1 - tail] == after[after.length - 1 - tail]) {
      tail++;
    }
    return (head, tail);
  }
}
