/// The three numbers the queue prints: a countdown, a size and an elapsed
/// time. Formatting only, no widgets, so the row and the card agree and a
/// test can check the strings without a tree.
library;

/// [duration] as `mm:ss`, never negative.
///
/// Minutes are not wrapped at sixty: a hold budget of two hours reads
/// `120:00`, which is longer but never wrong. The digits are tabular
/// (`HType.monoFeatures`), so the text does not jitter while it counts down.
String formatCountdown(Duration duration) {
  final int seconds = duration.isNegative ? 0 : duration.inSeconds;
  final String minutes = (seconds ~/ 60).toString().padLeft(2, '0');
  final String rest = (seconds % 60).toString().padLeft(2, '0');
  return '$minutes:$rest';
}

/// [bytes] as a short size: `0 B`, `512 B`, `1.2 kB`, `3.4 MB`.
///
/// Decimal units, because that is what the daemon logs and what a person
/// compares against a content length header.
String formatBytes(int bytes) {
  if (bytes < 1000) {
    return '$bytes B';
  }
  const List<String> units = <String>['kB', 'MB', 'GB', 'TB'];
  double value = bytes / 1000;
  int unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit++;
  }
  final String text = value >= 100
      ? value.round().toString()
      : value.toStringAsFixed(1);
  return '$text ${units[unit]}';
}
