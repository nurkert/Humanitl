/// The one notice the shell shows about the desktop side of this program
/// (HUM-034).
///
/// At most one at a time and never more than once for the same cause: a
/// desktop without a tray is a fact, and a fact is stated once. The person
/// closes it and the program does not bring it back.
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';

part 'notice.g.dart';

/// The diagnostic the shell shows above its sections, or null.
@Riverpod(keepAlive: true)
class AttentionNotice extends _$AttentionNotice {
  final Set<String> _shown = <String>{};

  @override
  Diagnostic? build() => null;

  /// Shows [diagnostic] unless one with the same code was shown before.
  void showOnce(Diagnostic diagnostic) {
    if (!_shown.add(diagnostic.code)) {
      return;
    }
    state = diagnostic;
  }

  /// Shows [diagnostic], replacing whatever stands.
  void show(Diagnostic diagnostic) => state = diagnostic;

  /// Takes the notice away.
  void dismiss() => state = null;
}
