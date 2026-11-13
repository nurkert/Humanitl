/// What the rows of the open overlay share: the write through `SetConfig`,
/// the daemon's refusals, the brief "Saved", and the anchors a jump lands on
/// (HUM-404).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/daemon_client.dart';
import '../../core/ui/ui.dart';

/// Writes one key through `SetConfig`; throws `DaemonException` on a refusal.
typedef SettingsWrite = Future<void> Function(String key, String value);

/// The state of the rows of one open overlay.
///
/// The overlay owns it; every row finds it through [SettingsScope]. The rows
/// keep no write logic of their own: a value goes to the daemon unchanged,
/// and what the daemon answers stands at the row (ADR-018).
class SettingsController extends ChangeNotifier {
  /// Creates the state; [_write] reaches the daemon.
  SettingsController(this._write);

  final SettingsWrite _write;

  final Map<String, Diagnostic> _refusals = <String, Diagnostic>{};
  final Set<String> _saved = <String>{};
  final Map<String, Timer> _savedTimers = <String, Timer>{};
  final Map<String, FocusNode> _anchors = <String, FocusNode>{};
  final Map<String, GlobalKey> _rows = <String, GlobalKey>{};
  String? _highlighted;
  Timer? _highlightTimer;
  bool _disposed = false;

  UnsavedGuard? _guard;

  /// Registers [guard] for a form of the page that can hold a change, or
  /// clears it with null. One form at a time: the page shows one category.
  void guardUnsaved(UnsavedGuard? guard) => _guard = guard;

  /// Clears the guard, but only if it is still [guard]: a page that goes
  /// does not drop the guard of the page that came after it.
  void releaseUnsaved(UnsavedGuard guard) {
    if (_guard == guard) {
      _guard = null;
    }
  }

  /// Runs [proceed] at once, or, while a form holds a change, after its own
  /// question was answered with save or discard. Closing the overlay and
  /// choosing another category go through here (HUM-414).
  void leave(VoidCallback proceed) {
    final UnsavedGuard? guard = _guard;
    if (guard == null || !guard(proceed)) {
      proceed();
    }
  }

  /// How long a row stays lit after a jump to it.
  static const Duration highlightFor = Duration(milliseconds: 1200);

  /// Writes [value] to [key]. On success the row says "Saved" for a moment
  /// and its last refusal goes; on a refusal the daemon's reason stands at
  /// the row instead.
  Future<void> write(String key, String value) async {
    try {
      await _write(key, value);
      if (_disposed) {
        return;
      }
      _refusals.remove(key);
      markSaved(key);
    } on DaemonException catch (error) {
      if (_disposed) {
        return;
      }
      _refusals[key] = error.diagnostic;
      _saved.remove(key);
      notifyListeners();
    }
  }

  /// Lets the row of [key] say "Saved" for [HMotion.copyFeedback]; for a row
  /// that writes through its own RPC, such as the scale.
  void markSaved(String key) {
    _saved.add(key);
    _savedTimers.remove(key)?.cancel();
    _savedTimers[key] = Timer(HMotion.copyFeedback, () {
      _savedTimers.remove(key);
      if (!_disposed && _saved.remove(key)) {
        notifyListeners();
      }
    });
    notifyListeners();
  }

  /// The daemon's refusal of the last write of [key], or null.
  Diagnostic? refusal(String key) => _refusals[key];

  /// Whether [key] was just written.
  bool saved(String key) => _saved.contains(key);

  /// The row a jump has just landed on, while it is lit.
  String? get highlighted => _highlighted;

  /// The setting a jump wants to land on while its row does not stand yet
  /// (HUM-405). A page that draws the row only after a step of its own, such
  /// as the Models category, which opens the detail of a server for the
  /// endpoint, reads it, takes that step and calls [reveal] itself. Set by
  /// the overlay without a notification: the page is built anew with it.
  String? get wanted => _wanted;
  String? _wanted;

  /// Notes [key] as the target of a jump; null when none waits.
  // ignore: use_setters_to_change_properties
  void want(String? key) => _wanted = key;

  /// The node around the control of [key]; it takes no focus itself, a jump
  /// hands the focus to the first control inside it (HUM-275).
  FocusNode anchor(String key) => _anchors.putIfAbsent(
    key,
    () => FocusNode(
      debugLabel: 'setting-anchor-$key',
      canRequestFocus: false,
      skipTraversal: true,
    ),
  );

  /// The key of the row of [key], for scrolling to it.
  GlobalKey rowKey(String key) =>
      _rows.putIfAbsent(key, () => GlobalKey(debugLabel: 'setting-row-$key'));

  /// Scrolls to the row of [key], focuses its control and lights it briefly.
  /// Returns false while the row is not built.
  bool reveal(String key) {
    final BuildContext? row = _rows[key]?.currentContext;
    if (row == null) {
      return false;
    }
    // Ohne Dauer: Der Sprung ist die Antwort auf einen Klick, keine
    // Bewegung, die etwas erklärt (`docs/UX.md` 2.1).
    unawaited(Scrollable.ensureVisible(row, alignment: 0.1));
    _wanted = null;
    _anchors[key]?.traversalDescendants.firstOrNull?.requestFocus();
    _highlighted = key;
    _highlightTimer?.cancel();
    _highlightTimer = Timer(highlightFor, () {
      if (!_disposed && _highlighted == key) {
        _highlighted = null;
        notifyListeners();
      }
    });
    notifyListeners();
    return true;
  }

  /// Hands the focus to the control of [key] once more, if its row is the
  /// one a jump just lit: for a row whose control appears only after an
  /// answer of the daemon, later than the jump (HUM-440).
  void refocus(String key) {
    if (_highlighted == key) {
      _anchors[key]?.traversalDescendants.firstOrNull?.requestFocus();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    for (final Timer timer in _savedTimers.values) {
      timer.cancel();
    }
    _highlightTimer?.cancel();
    for (final FocusNode node in _anchors.values) {
      node.dispose();
    }
    super.dispose();
  }
}

/// A form that would lose a change: given what should happen next, it
/// returns false when it holds no change, or true when it asks first and
/// runs `proceed` itself once the change was saved or discarded.
typedef UnsavedGuard = bool Function(VoidCallback proceed);

/// Puts the [SettingsController] of the open overlay above its rows.
class SettingsScope extends InheritedNotifier<SettingsController> {
  /// Puts [controller] above [child].
  const SettingsScope({
    required SettingsController controller,
    required super.child,
    super.key,
  }) : super(notifier: controller);

  /// The controller above [context], or null outside an overlay, as in a
  /// golden of a single section.
  static SettingsController? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<SettingsScope>()?.notifier;
}
