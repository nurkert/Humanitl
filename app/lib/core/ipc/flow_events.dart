/// The one `Subscribe` the application holds, and the reconnect around it.
///
/// Every screen is a projection of this stream: the queue folds it into a map,
/// the history updates the rows it has loaded, the tray counts what is waiting
/// (BACKLOG.md 5). It lives in `core` and not in a feature because a feature
/// may not import another feature (ARCHITECTURE 5) — and because a second
/// provider would mean a second `Subscribe`, so two screens would each see
/// half the events in the fake and double the traffic against the daemon.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../domain/domain.dart';
import 'client_providers.dart';
import 'connection.dart';
import 'daemon_client.dart';

/// The longest wait between two reconnect attempts.
const Duration maxReconnectBackoff = Duration(seconds: 30);

/// The first wait after the event stream failed; it doubles up to
/// [maxReconnectBackoff]. Tests override it to keep themselves short.
final Provider<Duration> reconnectBackoffProvider = Provider<Duration>(
  (Ref ref) => const Duration(seconds: 1),
);

/// `Subscribe`, kept alive across daemon restarts.
///
/// A broken stream is retried with 1 s, 2 s, 4 s ... up to
/// [maxReconnectBackoff]. Every connection starts with a synthetic
/// [FlowEvent.lagged], the first one included, because everything that
/// happened while the app was not listening is exactly what `Lagged` means;
/// the queue answers it with the same `ListFlows` resync it uses for a real
/// gap. The first connection needs it most: `Subscribe` without a
/// `since_flow_id` means "from now on", so without the resync a client that
/// starts while three requests are held never hears about them and every
/// screen shows an empty queue that is not empty (HUM-020, HUM-034).
///
/// Written with an explicit subscription rather than as an `async*` generator
/// so that `ref.onDispose` can cancel the source at once: a generator is only
/// cancelled when it next resumes, which leaves the daemon -- or the fake --
/// holding a timer nobody waits for.
///
/// **The backoff is not waited out once the line is demonstrably back.**
/// `GetInfo` and `Subscribe` are two calls over the same socket, and a daemon
/// that went away breaks both. The first one comes back through
/// [connectionStateProvider]'s two-second retry, and [linkLiveProvider] says
/// so; this stream would still be sitting on a wait that has doubled its way
/// up to [maxReconnectBackoff], and for up to half a minute the shell would
/// claim to be live while no event reaches it -- countdowns running on
/// requests that are already forwarded, a second `Enter` answered with
/// `FLOW_NOT_HELD`. The listener below therefore drops the pending wait and
/// connects at once. It does not make [linkLiveProvider] answer for this
/// stream as well: that provider keeps its one meaning, "does what is on
/// screen still come from the daemon", and this stream follows it rather than
/// asking a second question of its own. It is also set up only once the stream
/// is down, because that is the only state in which the answer changes
/// anything here.
final StreamProvider<FlowEvent> flowEventsProvider = StreamProvider<FlowEvent>((
  Ref ref,
) {
  final DaemonClient client = ref.watch(daemonClientProvider);
  final Duration base = ref.watch(reconnectBackoffProvider);
  final StreamController<FlowEvent> events = StreamController<FlowEvent>();
  StreamSubscription<FlowEvent>? source;
  Timer? retry;
  Duration wait = base;
  bool disposed = false;
  ProviderSubscription<bool>? link;

  // Der Rückfall ruft den Verbindungsversuch und der Verbindungsversuch den
  // Rückfall; eine der beiden muss deshalb eine Variable sein, die erst nach
  // der anderen gefüllt wird.
  late final void Function() scheduleReconnect;

  void connect() => connectFlowEvents(
    client: client,
    events: events,
    afterGap: true,
    onEvent: () => wait = base,
    onBroken: () => scheduleReconnect(),
    attach: (StreamSubscription<FlowEvent> subscription) =>
        source = subscription,
    isDisposed: () => disposed,
  );

  // Erst wenn der Strom wirklich unten ist, wird nach der Leitung gefragt --
  // und dann nur einmal. Ein Horcher, den dieser Provider unbedingt aufsetzte,
  // baute [connectionStateProvider] in jedem Baum, der Ereignisse liest, auch
  // in denen, die nie eine Verbindung aufgemacht haben; damit liefe deren
  // Herzschlag mit, ohne dass irgendjemand ihn bestellt hätte.
  void watchTheLink() {
    link ??= ref.listen<bool>(linkLiveProvider, (bool? previous, bool live) {
      if (!live || disposed || events.isClosed || source != null) {
        return;
      }
      retry?.cancel();
      retry = null;
      wait = base;
      connect();
    });
  }

  scheduleReconnect = () {
    source = null;
    if (disposed || events.isClosed) {
      return;
    }
    watchTheLink();
    retry?.cancel();
    retry = Timer(wait, () {
      retry = null;
      final Duration doubled = wait * 2;
      wait = doubled > maxReconnectBackoff ? maxReconnectBackoff : doubled;
      connect();
    });
  };

  ref.onDispose(() {
    disposed = true;
    retry?.cancel();
    link?.close();
    unawaited(source?.cancel());
    unawaited(events.close());
  });

  connect();
  return events.stream;
});

/// Subscribes [client] and pipes its events into [events].
///
/// Split out of [flowEventsProvider] so that the first attempt and every retry
/// take exactly the same path. [onBroken] runs when the stream fails,
/// [onEvent] when it delivers, and [attach] receives the live subscription so
/// the provider can cancel it.
void connectFlowEvents({
  required DaemonClient client,
  required StreamController<FlowEvent> events,
  required bool afterGap,
  required VoidCallback onEvent,
  required VoidCallback onBroken,
  required void Function(StreamSubscription<FlowEvent> subscription) attach,
  required bool Function() isDisposed,
}) {
  if (isDisposed() || events.isClosed) {
    return;
  }
  if (afterGap) {
    events.add(FlowEvent.lagged(at: DateTime.now(), dropped: 0));
  }
  try {
    attach(
      client.subscribe().listen(
        (FlowEvent event) {
          onEvent();
          if (!events.isClosed) {
            events.add(event);
          }
        },
        // Why the stream broke is the connection gate's business; the queue
        // only has to come back.
        onError: (Object error, StackTrace stack) => onBroken(),
        onDone: () {
          if (!isDisposed() && !events.isClosed) {
            events.close();
          }
        },
        cancelOnError: true,
      ),
    );
  } on Object {
    onBroken();
  }
}
