/// The pause of intercepting, as the daemon reports it (HUM-407).
///
/// A plain value type like the update status: a read-only view of an answer
/// of the daemon. Only the daemon holds the pause; the app shows it and asks
/// for a change over `Intercept` (ADR-018).
library;

/// Whether the daemon intercepts, mirror of `InterceptStatus`.
final class InterceptStatus {
  /// Creates a status; without [pausedSince] intercepting is on.
  const InterceptStatus({
    this.paused = false,
    this.pausedSince,
    this.revision = 0,
    this.instance = '',
  });

  /// Intercepting is on: every request no rule decides waits.
  static const InterceptStatus on = InterceptStatus();

  /// True while requests no rule decides pass without waiting.
  final bool paused;

  /// When the pause began, while [paused] is true.
  final DateTime? pausedSince;

  /// How many changes this daemon has seen; it rises by one with every
  /// change and starts at zero with every daemon. A client applies a status
  /// only when its revision is not below the highest it knows from the same
  /// daemon, so replies and events arriving out of order never undo a newer
  /// state (HUM-407).
  final int revision;

  /// The daemon this status comes from: the same value as its
  /// `Info.session_id`, given once at its start. A [revision] only means
  /// something together with it; empty from a daemon that does not say.
  final String instance;

  @override
  bool operator ==(Object other) =>
      other is InterceptStatus &&
      other.paused == paused &&
      other.pausedSince == pausedSince &&
      other.revision == revision &&
      other.instance == instance;

  @override
  int get hashCode => Object.hash(paused, pausedSince, revision, instance);

  @override
  String toString() =>
      'InterceptStatus(paused: $paused, since: $pausedSince, '
      'revision: $revision, instance: $instance)';
}

/// What an `Intercept` call asks the daemon to do.
enum InterceptOp {
  /// Only read the status.
  status,

  /// Pause intercepting.
  pause,

  /// Intercept again.
  resume,
}
