/// Which section the shell shows (`navigationProvider`, CONVENTIONS 3.9).
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../section.dart';

part 'navigation.g.dart';

/// The selected section. Starts on the queue, which is where the work is.
@Riverpod(keepAlive: true)
class Navigation extends _$Navigation {
  @override
  Section build() => Section.intercept;

  /// Shows [section].
  void go(Section section) => state = section;

  /// Shows the section at [index]; out-of-range indices are ignored so a
  /// stray `Ctrl+9` does nothing rather than throwing.
  void goIndex(int index) {
    if (index >= 0 && index < Section.values.length) {
      state = Section.values[index];
    }
  }
}
