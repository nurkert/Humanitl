/// Die Befunde des Ereignisstroms, wie der Intercept-Bildschirm sie hält
/// (HUM-045, HUM-106).
///
/// Der Daemon erkennt Dinge, die keine Entscheidung sind und trotzdem gesagt
/// werden müssen: einen abgelehnten TLS-Handschlag samt dem Werkzeug, dem das
/// Zertifikat fehlt (`TLS_001`), einen Handschlag ohne SNI (`TLS_003`). Bis zu
/// diesem Issue kamen sie an und wurden verworfen. Ein Programm, dessen Zweck
/// es ist, Netzverkehr erklärbar zu machen, darf den einen Satz nicht
/// wegwerfen, der erklärt, warum eine Anfrage gescheitert ist.
///
/// Drei Festlegungen:
///
/// * **Kein Zusammenfassen, aber eine Obergrenze.** Zwei Befunde sind zwei
///   Karten, auch bei gleichem Code: Der Daemon entstört `TLS_001..003` je
///   Host für 60 Sekunden und deckelt sie bei 32 je Fenster
///   (`tls_observe.rs`), und was er dort schon zusammengefasst hat, wird hier
///   nicht noch einmal zusammengefasst. **Diese Entstörung deckt aber nur die
///   TLS-Codes.** `LLM_005` entsteht je durchgereichter Anfrage mit Funden
///   (`pipeline.rs::warn_about_findings`), `PROXY_002` je widersprüchlicher
///   Zieladresse und `PROXY_005` je abgelehntem Übergang
///   (`handler.rs::publish_diagnostic`, `publish_invalid_transition`) — alle
///   drei ohne jedes Fenster und alle drei vom Agenten auslösbar. Eine Liste
///   ohne Grenze wäre damit ein Speicherleck, das der Agent selbst füllt.
///   [maxSessionDiagnostics] ist die Grenze; der älteste Eintrag fällt heraus,
///   und [Diagnostics.dropped] zählt mit, damit der Verlust auf dem Schirm
///   steht und nicht verschwiegen wird (`backlog/CONVENTIONS.md` 4.13).
/// * **Die Flusskennung reist mit.** `flow_diagnostic` nennt den Fluss, zu dem
///   der Befund gehört; ein Befund ohne Kennung (`diagnostic`, Feld 12) steht
///   für die Sitzung. Beide landen in derselben Liste, weil beide denselben
///   Ort brauchen: den Streifen über der Warteschlange. Die Kennung zu
///   verwerfen hieße, die Zuordnung zu verlieren, die der Daemon schon kennt.
/// * **Der fremde Text wird an genau dieser Grenze bereinigt.** Der Satz des
///   Daemons trägt Material von außen — einen Hostnamen aus dem Netz, den
///   Fehlertext einer Zertifikatsprüfung. Was diesen Notifier verlässt, hat
///   [sanitizeDiagnostic] passiert, dieselbe Bereinigung, die auch die
///   Rumpf-Ansichten benutzen; keine Ansicht muss sich später daran erinnern.
library;

import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/flow_events.dart';
import '../../../core/text/sanitize_diagnostic.dart';

part 'diagnostics.g.dart';

/// Ein Befund, wie der Streifen ihn hält.
@immutable
class SessionDiagnostic {
  /// Erzeugt einen Eintrag.
  const SessionDiagnostic({
    required this.id,
    required this.at,
    required this.diagnostic,
    this.flowId,
  });

  /// Die laufende Nummer dieser Sitzung.
  ///
  /// Der Code taugt nicht als Schlüssel: Derselbe Code kommt für zwei
  /// verschiedene Hosts zweimal, und beide Karten sollen stehen bleiben. Die
  /// Nummer ist auch das, was [Diagnostics.dismiss] entfernt.
  final int id;

  /// Wann der Daemon den Befund erhoben hat.
  final DateTime at;

  /// Der Fluss, zu dem der Befund gehört, oder null für die Sitzung.
  final FlowId? flowId;

  /// Der Befund selbst, bereits bereinigt.
  final Diagnostic diagnostic;

  @override
  bool operator ==(Object other) =>
      other is SessionDiagnostic &&
      other.id == id &&
      other.at == at &&
      other.flowId == flowId &&
      other.diagnostic == diagnostic;

  @override
  int get hashCode => Object.hash(id, at, flowId, diagnostic);
}

/// So viele Befunde hält der Streifen zugleich.
///
/// Genug für alles, was eine Sitzung von Hand erzeugt, und klein genug, dass
/// weder der Speicher noch die Liste daran wächst, wenn ein Agent im Takt
/// seiner Anfragen `LLM_005` auslöst.
const int maxSessionDiagnostics = 200;

/// Die Befunde dieser Sitzung, ältester zuerst.
@Riverpod(keepAlive: true)
class Diagnostics extends _$Diagnostics {
  int _next = 0;
  int _dropped = 0;

  /// Wie viele Befunde die Obergrenze aus der Liste gedrängt hat.
  ///
  /// Der Streifen schreibt die Zahl hin. Ein stilles Wegwerfen wäre die eine
  /// Sache, die dieses Issue verhindern soll.
  int get dropped => _dropped;

  @override
  List<SessionDiagnostic> build() {
    // `fireImmediately` startet den Strom; ein Zuhörer allein baut den
    // Provider nicht, auf den er hört (dieselbe Begründung wie in
    // `flows.dart` und `agent_asks.dart`).
    ref.listen(flowEventsProvider, (
      AsyncValue<FlowEvent>? previous,
      AsyncValue<FlowEvent> next,
    ) {
      next.whenData(_apply);
    }, fireImmediately: true);
    return const <SessionDiagnostic>[];
  }

  /// Alles andere gehört zu einem Fluss und wird in `flows.dart` gefaltet;
  /// dieser Notifier sieht auf eine Variante und lässt den Rest liegen.
  void _apply(FlowEvent event) {
    if (event case FlowEventDiagnostic(
      :final DateTime at,
      :final Diagnostic diagnostic,
      :final FlowId? flowId,
    )) {
      final List<SessionDiagnostic> next = <SessionDiagnostic>[
        ...state,
        SessionDiagnostic(
          id: _next++,
          at: at,
          flowId: flowId,
          diagnostic: sanitizeDiagnostic(diagnostic),
        ),
      ];
      if (next.length > maxSessionDiagnostics) {
        _dropped += next.length - maxSessionDiagnostics;
        next.removeRange(0, next.length - maxSessionDiagnostics);
      }
      state = next;
    }
  }

  /// Nimmt den Befund mit [id] vom Schirm.
  ///
  /// Das ist die Antwort „gesehen" eines Menschen und gilt nur in diesem
  /// Client: Der Daemon führt über einen Befund keinen Zustand, weil ein
  /// Befund nichts anhält. Ein späterer Befund desselben Codes ergibt eine
  /// neue Karte.
  void dismiss(int id) {
    state = <SessionDiagnostic>[
      for (final SessionDiagnostic open in state)
        if (open.id != id) open,
    ];
  }
}
