/// The note that travels with a block (HUM-072).
///
/// The daemon sanitises the note before it writes it into the `403` body and
/// into the `x-humanitl-note` header (`humanitl_core::block`). The same rules
/// are implemented here so that the action bar can show what the agent will
/// read instead of what was typed: a field that shows one text while the
/// agent receives another would be exactly the kind of claim
/// `backlog/CONVENTIONS.md` 4.13 forbids.
///
/// Nothing here draws, so a unit test can compare every rule with the Rust
/// side without a widget tree.
library;

/// How many characters of a note go out at most; `NOTE_MAX_CHARS` of the
/// daemon.
const int noteMaxChars = 500;

/// The line the note takes in the `403` body, without its newline.
const String noteBodyPrefix = 'note: ';

/// The header the note additionally travels in.
const String noteHeaderName = 'x-humanitl-note';

/// The note as the agent reads it in the body of the `403`.
///
/// Carriage return, line feed and the two Unicode line separators become a
/// space, so that a note can neither open a second header line nor imitate
/// the structure of the body. Tab survives, every other control character and
/// every invisible character is dropped, runs of spaces collapse, the ends are
/// trimmed and at most [noteMaxChars] characters remain. Non-ASCII stays;
/// [noteHeaderValue] shortens it further for the header.
String sanitizeNote(String note) {
  final StringBuffer out = StringBuffer();
  bool lastWasSpace = false;
  for (final int rune in note.runes) {
    final int? kept = switch (rune) {
      0x0d || 0x0a || 0x2028 || 0x2029 => 0x20,
      0x09 => 0x09,
      _ when _isControl(rune) || _isInvisible(rune) => null,
      _ when _isWhitespace(rune) => 0x20,
      _ => rune,
    };
    if (kept == null) {
      continue;
    }
    if (kept == 0x20) {
      if (lastWasSpace) {
        continue;
      }
      lastWasSpace = true;
    } else {
      lastWasSpace = false;
    }
    out.writeCharCode(kept);
  }
  final List<int> runes = out.toString().trim().runes.toList();
  final String capped = String.fromCharCodes(
    runes.length <= noteMaxChars ? runes : runes.sublist(0, noteMaxChars),
  );
  return capped.trimRight();
}

/// The note as it fits into a header value.
///
/// RFC 9110 §5.5 allows visible ASCII plus space and tab between them;
/// everything else is dropped, so non-ASCII reaches the agent in the body
/// only. Expects the output of [sanitizeNote].
String noteHeaderValue(String note) {
  final StringBuffer out = StringBuffer();
  for (final int rune in note.runes) {
    if (rune == 0x20 || rune == 0x09 || (rune >= 0x21 && rune <= 0x7e)) {
      out.writeCharCode(rune);
    }
  }
  return out.toString().trim();
}

/// True when the header carries less than the body does, because the note
/// holds characters a field value may not.
///
/// The bar says so in one sentence instead of silently sending two different
/// texts (`docs/UX.md` 4.13).
bool noteLosesCharactersInHeader(String sanitized) =>
    noteHeaderValue(sanitized) != sanitized;

/// Unicode `Cc`: the C0 and C1 control characters.
bool _isControl(int rune) => rune < 0x20 || (rune >= 0x7f && rune <= 0x9f);

/// Unicode `White_Space`, minus what the caller handled already.
bool _isWhitespace(int rune) =>
    rune == 0x20 ||
    rune == 0xa0 ||
    rune == 0x1680 ||
    (rune >= 0x2000 && rune <= 0x200a) ||
    rune == 0x202f ||
    rune == 0x205f ||
    rune == 0x3000;

/// Characters that show nothing and can make a note read differently in the
/// terminal of the agent than it does in the field: zero width and bidi.
bool _isInvisible(int rune) =>
    rune == 0x00ad ||
    rune == 0x034f ||
    rune == 0x061c ||
    rune == 0x180e ||
    (rune >= 0x200b && rune <= 0x200f) ||
    (rune >= 0x202a && rune <= 0x202e) ||
    (rune >= 0x2060 && rune <= 0x2064) ||
    (rune >= 0x2066 && rune <= 0x2069) ||
    rune == 0xfeff ||
    (rune >= 0xe0000 && rune <= 0xe007f);
