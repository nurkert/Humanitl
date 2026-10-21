/// Ob die Aufzeichnung gerade schreibt, wie der Daemon es sagt (HUM-250).
///
/// Die Sicherheitsaussage in `README.md` hängt an der Aufzeichnung: Was
/// entschieden wird, ist danach nachlesbar. Ist die Platte voll oder die
/// Datenbank gesperrt, gilt das nicht mehr, und die Oberfläche darf dann
/// nicht weiter so aussehen, als würde alles aufgezeichnet.
///
/// Der Zustand kommt aus dem Ereignisstrom und wird hier nicht ausgerechnet
/// (ADR-018). Der Daemon meldet den Beginn einer Störung als Befund der
/// Sitzung mit `RECORDER_001` oder `RECORDER_003`, einmal je Ursache, und ihr
/// Ende mit `RECORDER_005`. Einem Client, der sich während der Störung
/// verbindet, liefert er den Befund vor allem anderen nach. Daraus folgen
/// drei Regeln:
///
/// * **Beginn:** ein Befund ohne Flow mit einem der beiden Codes und
///   mindestens `error`. Eine Warnung desselben Codes, etwa über die Rechte
///   einer Datei, lässt die Aufzeichnung laufen und zählt nicht.
/// * **Ende:** `RECORDER_005`.
/// * **Neue Verbindung:** Das erste Ereignis jeder Verbindung ist ein
///   künstliches `Lagged` mit null verlorenen Ereignissen
///   (`connectFlowEvents`). Dort wird der Stand vergessen; läuft die Störung
///   noch, sagt der Daemon es gleich danach. Ein Daemon, der inzwischen neu
///   gestartet ist und wieder schreibt, schickt kein `RECORDER_005`, und ohne
///   diese Regel bliebe die Leiste für immer rot.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:humanitl_ui/humanitl_ui.dart' show HMotion;
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/flow_events.dart';
import 'diagnostics.dart' show sanitizeDiagnostic;

part 'recorder_health.g.dart';

/// Eine laufende Störung der Aufzeichnung.
@immutable
class RecorderOutage {
  /// Erzeugt den Stand.
  const RecorderOutage({required this.since, required this.diagnostic});

  /// Wann der Daemon den ersten Befund dieser Störung erhob.
  final DateTime since;

  /// Der erste Befund dieser Störung, bereinigt.
  final Diagnostic diagnostic;

  @override
  bool operator ==(Object other) =>
      other is RecorderOutage &&
      other.since == since &&
      other.diagnostic == diagnostic;

  @override
  int get hashCode => Object.hash(since, diagnostic);
}

/// Wahr, wenn [diagnostic] als Befund der Sitzung eine Störung der
/// Aufzeichnung beginnt.
bool startsRecorderOutage(Diagnostic diagnostic) =>
    (diagnostic.code == DiagnosticCodes.recorderUnavailable ||
        diagnostic.code == DiagnosticCodes.recorderWriteFailed) &&
    (diagnostic.severity == Severity.error ||
        diagnostic.severity == Severity.blocking);

/// Die laufende Störung der Aufzeichnung, oder null, solange sie schreibt.
@Riverpod(keepAlive: true)
class RecorderHealth extends _$RecorderHealth {
  @override
  RecorderOutage? build() {
    // `fireImmediately` startet den Strom (dieselbe Begründung wie in
    // `diagnostics.dart`).
    ref.listen(flowEventsProvider, (
      AsyncValue<FlowEvent>? previous,
      AsyncValue<FlowEvent> next,
    ) {
      next.whenData(_apply);
    }, fireImmediately: true);
    return null;
  }

  /// Wahr nach einem echten Überlauf, bis das nächste Ereignis zeigt, ob
  /// die Störung noch läuft.
  bool _afterGap = false;

  void _apply(FlowEvent event) {
    // Hinter einem echten `Lagged` liefert der Daemon die laufende Störung
    // sofort nach (`event_stream`, HUM-250, Review). Kommt stattdessen etwas
    // anderes, lief keine mehr, und ihr `RECORDER_005` war unter dem
    // Verlorenen. Gelöscht wird erst dann und nicht schon beim `Lagged`:
    // Eine offene Pause bliebe sonst nicht stehen.
    //
    // Eine Regeländerung entscheidet das nicht: Sie kommt aus einem eigenen
    // Strom des Daemons, und die Oberfläche verlässt sich nicht darauf, dass
    // sie nie zwischen `Lagged` und die nachgelieferte Störung fällt (Review
    // Runde 2). Jedes andere Ereignis kommt aus demselben Rundfunk wie `Lagged`,
    // hinter dem die Störung unmittelbar folgt; auf es zu warten hieße, eine
    // vorbeigegangene Störung so lange rot zu zeigen, bis zufällig ein Befund
    // kommt. Ein doppelt gelieferter Befund derselben Störung ändert den
    // Stand nicht (`state == null` unten).
    if (_afterGap &&
        event is! FlowEventLagged &&
        event is! FlowEventRulesChanged) {
      _afterGap = false;
      if (!(event is FlowEventDiagnostic &&
          event.flowId == null &&
          startsRecorderOutage(event.diagnostic))) {
        state = null;
      }
    }
    switch (event) {
      case FlowEventLagged(dropped: 0):
        _afterGap = false;
        state = null;
      case FlowEventLagged():
        _afterGap = true;
      case FlowEventDiagnostic(
        flowId: null,
        :final DateTime at,
        :final Diagnostic diagnostic,
      ):
        if (diagnostic.code == DiagnosticCodes.recorderRecovered) {
          state = null;
        } else if (state == null && startsRecorderOutage(diagnostic)) {
          state = RecorderOutage(
            since: at,
            diagnostic: sanitizeDiagnostic(diagnostic),
          );
        }
      case _:
        break;
    }
  }
}

/// Wie weit die Pause ist.
enum RecorderPauseState {
  /// Keine Pause offen.
  closed,

  /// Offen, und „Unaufgezeichnet senden" nimmt noch nicht an.
  arming,

  /// Offen, und der Knopf nimmt an.
  armed;

  /// Wahr, solange die Pause steht.
  bool get isOpen => this != closed;
}

/// Die Bestätigungs-Pause vor der ersten Freigabe während einer Störung
/// (HUM-250, `docs/UX.md` 5.4).
///
/// Solange die Aufzeichnung gestört ist, geht die erste Freigabe dieser
/// Sitzung nicht sofort hinaus: An der Stelle der Entscheidungsleiste steht
/// eine Pause, die sagt, dass nichts davon in der History landet. Wer dort
/// „Unaufgezeichnet senden" wählt, hat es gelesen; danach gilt die
/// Bestätigung für den Rest der Sitzung, und jede Freigabe geht wieder wie
/// gewohnt. Kein Modal: Der Schutz sitzt in der Zeit. Der Knopf nimmt erst
/// [HMotion.rearm] nach dem Aufgehen an, damit ein zweites `Enter` oder `S`
/// die Pause nicht ungelesen überspringt.
///
/// Was nach der Bestätigung geschieht, reicht der Aufrufer beim Öffnen
/// herein: genau die Freigabe, die angehalten wurde, mit denselben
/// Argumenten. Als bestätigt gilt die Sitzung erst, wenn diese Freigabe
/// wirklich hinausging ([noteAllowSent]); weist sie sich selbst ab, steht die
/// Pause beim nächsten Versuch wieder da. „Sitzung" ist die des Daemons:
/// Nach einem Neustart fragt die Pause von vorn.
@Riverpod(keepAlive: true)
class RecorderPause extends _$RecorderPause {
  bool _acknowledged = false;
  bool _resuming = false;
  String? _session;
  Timer? _arming;
  Future<void> Function()? _resume;

  @override
  RecorderPauseState build() {
    ref.onDispose(() => _arming?.cancel());
    // Schreibt die Aufzeichnung wieder, gibt es nichts mehr zu bestätigen;
    // die angehaltene Freigabe geht dann den gewohnten Weg, wenn jemand sie
    // erneut auslöst.
    ref.listen<RecorderOutage?>(recorderHealthProvider, (
      RecorderOutage? previous,
      RecorderOutage? next,
    ) {
      if (next == null && state.isOpen) {
        close();
      }
    });
    return RecorderPauseState.closed;
  }

  /// Wahr, wenn eine Freigabe hier anhalten muss: Die Aufzeichnung ist
  /// gestört, und in dieser Sitzung wurde das noch nicht bestätigt.
  bool get owed {
    _followSession();
    return !_acknowledged &&
        !_resuming &&
        ref.read(recorderHealthProvider) != null;
  }

  /// Vergisst die Bestätigung, sobald der Daemon eine andere Sitzung nennt.
  ///
  /// Gefragt wird nur, wo es die Verbindung schon gibt: Ein Baum ohne sie
  /// baute sonst ihren Herzschlag mit (dieselbe Regel wie in
  /// `flow_events.dart`).
  void _followSession() {
    if (!ref.exists(connectionStateProvider)) {
      return;
    }
    final String? session = ref.read(connectionStateProvider).info?.sessionId;
    if (session != null && session != _session) {
      _session = session;
      _acknowledged = false;
    }
  }

  /// Eine Freigabe ging während der Störung hinaus. Kam sie aus der Pause,
  /// gilt die Sitzung damit als bestätigt.
  void noteAllowSent() {
    if (_resuming) {
      _acknowledged = true;
    }
  }

  /// Wahr, wenn die offene Pause zur Auswahl gehört und mit ihr geht.
  bool _bound = true;

  /// Öffnet die Pause; [resume] ist die angehaltene Freigabe.
  ///
  /// [bound] sagt, ob sie zur Auswahl gehört: Eine einzelne Freigabe hält
  /// über der Anfrage, die gerade ausgewählt ist, und ihre Pause geht, wenn
  /// die Auswahl weiterzieht. Ein Stapel umfasst mehrere Anfragen; seine
  /// Pause bleibt stehen, auch wenn die erste schon hinausging und der Cursor
  /// deshalb weiterrückte (HUM-250, Review).
  void open(Future<void> Function() resume, {bool bound = true}) {
    _resume = resume;
    _bound = bound;
    _arming?.cancel();
    _arming = Timer(HMotion.rearm, () {
      if (ref.mounted && state == RecorderPauseState.arming) {
        state = RecorderPauseState.armed;
      }
    });
    state = RecorderPauseState.arming;
  }

  /// Schließt sie, wenn sie zur Auswahl gehört ([open]); die Auswahl ist
  /// weitergezogen.
  void selectionMoved() {
    if (_bound) {
      close();
    }
  }

  /// Schließt sie, ohne etwas zu senden.
  void close() {
    _arming?.cancel();
    _resume = null;
    state = RecorderPauseState.closed;
  }

  /// „Unaufgezeichnet senden": setzt die angehaltene Freigabe fort, ohne
  /// erneut anzuhalten, und bestätigt die Sitzung, sobald sie hinausgeht
  /// ([noteAllowSent]). Tut nichts, solange die Pause nicht offen ist oder
  /// noch nicht [HMotion.rearm] stand.
  Future<void> confirm() async {
    final Future<void> Function()? resume = _resume;
    if (state != RecorderPauseState.armed || resume == null) {
      return;
    }
    close();
    _resuming = true;
    try {
      await resume();
    } finally {
      _resuming = false;
    }
  }
}
