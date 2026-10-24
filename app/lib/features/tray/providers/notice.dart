/// The one notice the shell shows about the desktop side of this program
/// (HUM-034).
///
/// At most one at a time and never more than once for the same cause: a
/// desktop without a tray is a fact, and a fact is stated once. The person
/// closes it and the program does not bring it back.
///
/// A notice raised through [AttentionNotice.showOnce] is such a fact, and
/// since HUM-276 its dismissal survives a restart. The shell keeps that
/// memory (`features/shell/providers/notice_dismissal.dart`), because it is
/// the shell that owns the state file; this notifier only says whether the
/// standing notice is one that should be remembered ([AttentionNotice.lasting]).
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';

part 'notice.g.dart';

/// The diagnostic the shell shows above its sections, or null.
@Riverpod(keepAlive: true)
class AttentionNotice extends _$AttentionNotice {
  final Set<String> _shown = <String>{};

  bool _lasting = false;

  @override
  Diagnostic? build() => null;

  /// True while the standing notice came through [showOnce]: a fact about
  /// the desktop, whose dismissal is kept across restarts.
  ///
  /// A notice from [show] is about one event (a notification answered too
  /// late, a link that went down). Closing it must not silence the next event
  /// with the same code.
  bool get lasting => state != null && _lasting;

  /// Shows [diagnostic] unless one with the same code was shown before.
  void showOnce(Diagnostic diagnostic) {
    if (!_shown.add(diagnostic.code)) {
      return;
    }
    _lasting = true;
    state = diagnostic;
  }

  /// Shows [diagnostic], replacing whatever stands.
  void show(Diagnostic diagnostic) {
    _lasting = false;
    state = diagnostic;
  }

  /// Takes the notice away.
  void dismiss() {
    _lasting = false;
    state = null;
  }
}
