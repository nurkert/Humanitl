/// What the last "Check" of each saved model server answered in this app
/// session (HUM-405).
///
/// The cards of the Models category show it, the detail view writes it. It
/// lives as long as the app and no longer: a check is a measurement of one
/// moment, and the daemon keeps none. A result belongs to the address it was
/// measured at; a card whose server has another address now shows none.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';

/// One answer of "Check".
final class ServerCheck {
  /// A check that found [models] models at [endpoint].
  const ServerCheck.found({required this.endpoint, required this.models})
    : failure = null;

  /// A check at [endpoint] that failed with [failure].
  const ServerCheck.failed({
    required this.endpoint,
    required Diagnostic this.failure,
  }) : models = 0;

  /// The address the check went to.
  final String endpoint;

  /// How many models the server offered.
  final int models;

  /// Why it failed, or null when it answered.
  final Diagnostic? failure;

  /// True when the server answered with a model list.
  bool get ok => failure == null;
}

/// The last check of each server, by name.
final NotifierProvider<ServerChecks, Map<String, ServerCheck>>
serverChecksProvider = NotifierProvider<ServerChecks, Map<String, ServerCheck>>(
  ServerChecks.new,
);

/// The notifier behind [serverChecksProvider].
class ServerChecks extends Notifier<Map<String, ServerCheck>> {
  /// No check yet.
  @override
  Map<String, ServerCheck> build() => const <String, ServerCheck>{};

  /// Notes [check] as the last one of [server].
  void record(String server, ServerCheck check) =>
      state = <String, ServerCheck>{...state, server: check};

  /// The last check of [server] at [endpoint], or null.
  ServerCheck? of(String server, String endpoint) {
    final ServerCheck? check = state[server];
    return check != null && check.endpoint == endpoint ? check : null;
  }

  /// Moves the result of [from] to [to] after a rename.
  void renamed(String from, String to) {
    final ServerCheck? check = state[from];
    if (check == null) {
      return;
    }
    state = <String, ServerCheck>{
      for (final MapEntry<String, ServerCheck> entry in state.entries)
        if (entry.key != from) entry.key: entry.value,
      to: check,
    };
  }
}
