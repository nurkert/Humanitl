/// The shared clock of the queue: one ticker for every countdown instead of a
/// timer per row (HUM-020 Fallstricke). Rows and rings watch [nowProvider];
/// nothing else in the app owns a periodic timer for the same purpose.
library;

import 'dart:async';

import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'now.g.dart';

/// How often [Now] publishes a new time.
///
/// 250 ms is fast enough for a countdown in `mm:ss` and slow enough that a
/// queue of two hundred rows does not repaint itself to death. Tests override
/// it, golden tests replace [nowProvider] outright so that no timer runs.
@Riverpod(keepAlive: true)
Duration nowInterval(Ref ref) => const Duration(milliseconds: 250);

/// The current time, republished every [nowIntervalProvider].
///
/// The timer starts with the first watcher and stops with the last one,
/// because the provider is disposed with the scope that holds it.
@Riverpod(keepAlive: true)
class Now extends _$Now {
  @override
  DateTime build() {
    final Timer timer = Timer.periodic(
      ref.watch(nowIntervalProvider),
      (Timer _) => state = DateTime.now(),
    );
    ref.onDispose(timer.cancel);
    return DateTime.now();
  }
}
