/// Which notices the person has closed for good (HUM-276).
///
/// The desktop notice of the shell (`UI_002`, "This desktop has no tray")
/// states a fact, and a fact stated once and closed should not come back on
/// the next start. The dismissal stands in the same small state file as the
/// width of the rail (`UiStateFile`, HUM-254), one flag per diagnostic code
/// under [dismissedNoticeKey].
///
/// Closing the banner hides the banner and nothing else. The doctor keeps its
/// own tray check, and the setup screen shows it as before: the fact is not
/// forgotten, only no longer announced. Tray detection itself is untouched.
///
/// A write that does not go through leaves the dismissal in memory for this
/// run, as everywhere in that file: a convenience fails like one.
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../intercept/providers/coach_mark.dart';

part 'notice_dismissal.g.dart';

/// The common start of every key this store writes.
const String dismissedNoticePrefix = 'noticeDismissed.';

/// The key under which the dismissal of the notice with [code] stands in the
/// state file, for example `noticeDismissed.UI_002`.
String dismissedNoticeKey(String code) => '$dismissedNoticePrefix$code';

/// The codes whose notice the person has closed for good.
///
/// Read once, synchronously, at the first build, for the reason the rail
/// gives: asynchronously the notice would flash for a frame before the
/// answer arrived.
@Riverpod(keepAlive: true)
class DismissedNotices extends _$DismissedNotices {
  @override
  Set<String> build() => <String>{
    for (final MapEntry<String, Object?> entry
        in ref.read(uiStateFileProvider).read().entries)
      if (entry.key.startsWith(dismissedNoticePrefix) && entry.value == true)
        entry.key.substring(dismissedNoticePrefix.length),
  };

  /// Remembers that the notice with [code] was closed, and writes it down.
  void remember(String code) {
    if (state.contains(code)) {
      return;
    }
    state = <String>{...state, code};
    ref.read(uiStateFileProvider).setBool(dismissedNoticeKey(code), true);
  }
}
