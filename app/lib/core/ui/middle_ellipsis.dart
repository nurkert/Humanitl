/// Shortening in the middle: a path keeps its start and its file name, a host
/// keeps its start and its apex. `TextOverflow.ellipsis` cuts the end, which
/// is the part that carries the meaning (BACKLOG.md 5, Anti-Patterns).
library;

/// The ellipsis character used by [middleEllipsis].
const String ellipsisChar = '…';

/// [text] shortened to at most [maxChars] characters by replacing its middle
/// with `…`. The text is returned unchanged when it already fits; [maxChars]
/// below 3 yields the ellipsis alone.
///
/// Counted in Unicode code points, so a surrogate pair is never split.
///
/// The ellipsis is charged to the start: whatever the budget allows goes to
/// the end, which is the half that identifies a path or a host.
///
/// `middleEllipsis('/very/long/path/to/file.json', 16)` is `'/very/…file.json'`.
String middleEllipsis(String text, int maxChars) {
  final List<int> runes = text.runes.toList(growable: false);
  if (runes.length <= maxChars) {
    return text;
  }
  if (maxChars < 3) {
    return ellipsisChar;
  }
  final int keep = maxChars - 1;
  final int head = (keep ~/ 2 - 1).clamp(0, keep);
  final int tail = keep - head;
  return String.fromCharCodes(runes.take(head)) +
      ellipsisChar +
      String.fromCharCodes(runes.skip(runes.length - tail));
}
