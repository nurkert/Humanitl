/// How full the hold queue is, against both of its limits (HUM-057, row 16
/// of the state table in HUM-058).
///
/// Der Daemon hält höchstens `limits.hold_max_flows` Anfragen mit zusammen
/// höchstens `limits.hold_max_bytes` Bytes; wer darüber kommt, wird mit `503`
/// geblockt und bekommt `LIMIT_002`. Die Statusleiste sagt das vorher: ab 80 %
/// einer der beiden Grenzen amber, ab 100 % rot. Rot ist hier richtig und
/// keine Warnfarbe: Bei voller Warteschlange werden neue Anfragen tatsächlich
/// geblockt (`docs/UX.md` 3.3, Rot heißt blockiert).
///
/// Die Zahl der Flüsse ist die der gehaltenen Flüsse, die die Oberfläche
/// ohnehin führt. Die Bytes rechnet die Oberfläche **nicht** selbst: Der
/// Daemon zählt im Byte-Budget gehaltene Rümpfe **und** die, die gerade
/// gepuffert oder gestreamt werden, und einen gehaltenen Kopf unter
/// `stream: true` nur mit seinem Präfix (HUM-057). Eine Summe der angekündigten
/// `requestSize` wiche in beide Richtungen ab: keine Warnung vor einem `503`,
/// und Rot für einen Kopf, der fast nichts belegt. Angezeigt wird deshalb
/// `Held.queue_bytes`, die Belegung, an der der Daemon ablehnt, in ihrem
/// letzten bekannten Stand ([daemonQueueBytesProvider]).
library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/connection.dart';
import '../../core/ipc/flow_events.dart';

/// The last occupancy of the daemon's byte budget, from `Held.queue_bytes`.
///
/// Kein Ereignis trägt die Zahl nach einer Entscheidung; sie ist der Stand
/// beim letzten Halten. Die Statusleiste nimmt sie nur, solange Flüsse
/// gehalten werden, damit ein veralteter Wert nicht stehen bleibt, wenn die
/// Warteschlange leer ist.
final NotifierProvider<DaemonQueueBytes, int> daemonQueueBytesProvider =
    NotifierProvider<DaemonQueueBytes, int>(DaemonQueueBytes.new);

/// Keeps [daemonQueueBytesProvider] at the last `Held.queue_bytes`.
class DaemonQueueBytes extends Notifier<int> {
  @override
  int build() {
    // Bricht die Verbindung ab oder kommt sie zurück, gilt der alte Wert
    // nicht mehr: Ein neu gestarteter Daemon fängt bei null an, und eine
    // Zahl von vor dem Bruch stünde sonst als Rot da. Eine laufende Belegung
    // per Ereignis oder RPC gehört zu HUM-258.
    ref.watch(linkLiveProvider);
    ref.listen<AsyncValue<FlowEvent>>(flowEventsProvider, (
      AsyncValue<FlowEvent>? previous,
      AsyncValue<FlowEvent> next,
    ) {
      if (next.value case final FlowEventHeld held) {
        state = held.queueBytes;
      }
    });
    return 0;
  }
}

/// How full the queue is.
enum QueuePressure {
  /// Under 80 % of both limits; the status bar says nothing.
  normal,

  /// At least 80 % of one limit.
  high,

  /// One limit reached; new requests are blocked with `LIMIT_002`.
  full,
}

/// Der Wert von `limits.hold_max_flows`, wenn der Daemon keinen nennt.
const int defaultHoldMaxFlows = 200;

/// Der Wert von `limits.hold_max_bytes`, wenn der Daemon keinen nennt.
const int defaultHoldMaxBytes = 256 * 1024 * 1024;

/// The load of the queue: what is held, and the two limits.
@immutable
class QueueLoad {
  /// Creates a load.
  const QueueLoad({
    required this.count,
    required this.bytes,
    required this.maxFlows,
    required this.maxBytes,
  });

  /// The load of [held] and the daemon's byte occupancy [poolBytes] against
  /// the limits in [config]. Without held flows the bytes count as zero: the
  /// last known occupancy is stale then.
  factory QueueLoad.of(List<Flow> held, ConfigValues config, int poolBytes) {
    int limit(String key, int fallback) => switch (config.at(key)) {
      final int value when value > 0 => value,
      _ => fallback,
    };
    return QueueLoad(
      count: held.length,
      bytes: held.isEmpty ? 0 : poolBytes,
      maxFlows: limit('limits.hold_max_flows', defaultHoldMaxFlows),
      maxBytes: limit('limits.hold_max_bytes', defaultHoldMaxBytes),
    );
  }

  /// How many requests are held.
  final int count;

  /// The occupancy of `limits.hold_max_bytes` as the daemon counted it: held
  /// bodies plus bodies being received or streamed.
  final int bytes;

  /// `limits.hold_max_flows`.
  final int maxFlows;

  /// `limits.hold_max_bytes`.
  final int maxBytes;

  /// How full the queue is: the fuller of the two limits decides.
  QueuePressure get pressure {
    if (count >= maxFlows || bytes >= maxBytes) {
      return QueuePressure.full;
    }
    // Ganzzahlig statt mit Gleitkomma: 80 % sind `x * 10 >= max * 8`.
    if (count * 10 >= maxFlows * 8 || bytes * 10 >= maxBytes * 8) {
      return QueuePressure.high;
    }
    return QueuePressure.normal;
  }
}
