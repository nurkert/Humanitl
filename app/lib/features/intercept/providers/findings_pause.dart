/// Über welcher Anfrage die Pause mit offenen Funden steht (HUM-049).
///
/// Ein Klick auf die amberfarbene Freigabe, `Enter` oder `A` senden eine
/// Anfrage mit einem offenen Fund nicht. Sie öffnen in der Karte eine Pause,
/// die aufzählt, was hinausginge, und drei Wege weiter anbietet: trotzdem
/// senden, pseudonymisieren, blockieren. Dieser Provider sagt nur, über
/// welcher Anfrage die Pause offen ist; was in ihr entschieden wird, läuft wie
/// jede andere Entscheidung dieses Bildschirms durch den Notifier in
/// `providers/decision.dart`.
///
/// Eine Flow-Id und kein Schalter: Die Pause gehört zu genau einer Anfrage.
/// Ein Schalter bliebe stehen, wenn die Auswahl weiterzieht, und öffnete die
/// Pause über der nächsten Anfrage, nach der niemand gefragt hat.
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';

part 'findings_pause.g.dart';

/// Die Anfrage, über der die Pause offen ist, oder null.
@Riverpod(keepAlive: true)
class OpenFindingsPause extends _$OpenFindingsPause {
  @override
  FlowId? build() => null;

  /// Öffnet die Pause über [id].
  void open(FlowId id) => state = id;

  /// Schließt sie; Schließen entscheidet nichts.
  void close() => state = null;
}
