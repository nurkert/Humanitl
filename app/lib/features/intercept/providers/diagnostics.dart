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
/// * **Zusammengefasst wird nur `LLM_005`, und eine Obergrenze gilt immer.**
///   Zwei Befunde sind zwei Karten, auch bei gleichem Code: Der Daemon
///   entstört `TLS_001..003` je Host für 60 Sekunden und deckelt sie bei 32 je
///   Fenster (`tls_observe.rs`), und was er dort schon zusammengefasst hat,
///   wird hier nicht noch einmal zusammengefasst. Die eine Ausnahme ist
///   `LLM_005` (HUM-354): Der Daemon warnt nur noch, wenn eine durchgereichte
///   Anfrage einen Fund trägt, vor dem er in diesem Lauf noch nicht oder vor
///   mehr als fünfzehn Minuten gewarnt hat (`pipeline.rs::warn_about_findings`),
///   aber ein Agent, der laufend Neues in sein Gespräch holt, löst sie
///   trotzdem im Takt seiner Anfragen aus. Folgt eine solche Warnung
///   unmittelbar auf eine für denselben Host und steht deren Karte noch, wird
///   daraus eine Karte mit Zähler ([SessionDiagnostic.requests],
///   [SessionDiagnostic.newFindings]); sie trägt den Satz und den Fluss der
///   jüngsten und rückt ans Ende. Eine Warnung für einen anderen Host
///   unterbricht die Folge, auch wenn ihre Karte schon ausgeblendet ist. `PROXY_002` entsteht je widersprüchlicher
///   Zieladresse und `PROXY_005` je abgelehntem Übergang
///   (`handler.rs::publish_diagnostic`, `publish_invalid_transition`), beide
///   ohne jedes Fenster und beide vom Agenten auslösbar. Eine Liste ohne
///   Grenze wäre damit ein Speicherleck, das der Agent selbst füllt.
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
    this.requests = 1,
    this.newFindings,
    this.llmHost,
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
  ///
  /// Bei einer zusammengefassten `LLM_005`-Karte der jüngste.
  final Diagnostic diagnostic;

  /// Wie viele Anfragen diese Karte zusammenfasst; 1 für jeden gewöhnlichen
  /// Befund (HUM-354).
  final int requests;

  /// Wie viele neue Funde die zusammengefassten `LLM_005` gemeldet haben,
  /// zusammengezählt; null bei jedem anderen Befund.
  final int? newFindings;

  /// Der Host des Sprachmodells, für den diese `LLM_005`-Karte steht; null
  /// bei jedem anderen Befund und bei einem Satz, den [llmWarningOf] nicht
  /// lesen kann. Nur Karten mit Host werden zusammengefasst.
  final String? llmHost;

  @override
  bool operator ==(Object other) =>
      other is SessionDiagnostic &&
      other.id == id &&
      other.at == at &&
      other.flowId == flowId &&
      other.diagnostic == diagnostic &&
      other.requests == requests &&
      other.newFindings == newFindings &&
      other.llmHost == llmHost;

  @override
  int get hashCode =>
      Object.hash(id, at, flowId, diagnostic, requests, newFindings, llmHost);
}

/// Was die Karte aus dem Satz einer `LLM_005` liest: Host und neue Funde.
typedef LlmWarning = ({String host, int newFindings});

/// Die Form, in der der Daemon `LLM_005` schreibt
/// (`daemon/crates/proxy/src/pipeline.rs`, `llm_warning_sentence`; dessen
/// Test `the_llm_warning_sentence_is_pinned` und die Testvorlage `llm005Why`
/// in `app/test/features/intercept/fixtures.dart` halten den Text fest).
final RegExp _llmWarningSentence = RegExp(
  r'language model at (\S+) carries [^:]*: (\d+) new, (\d+) in total',
);

/// Liest Host und Zahl der neuen Funde aus einer `LLM_005` (HUM-354).
///
/// Null für jeden anderen Code und für einen Satz in anderer Form, etwa von
/// einem Daemon vor HUM-354. Eine solche Karte bleibt einzeln stehen: Lieber
/// eine Karte zu viel als zwei Hosts in einer.
LlmWarning? llmWarningOf(Diagnostic diagnostic) {
  if (diagnostic.code != 'LLM_005') {
    return null;
  }
  final RegExpMatch? match = _llmWarningSentence.firstMatch(diagnostic.why);
  if (match == null) {
    return null;
  }
  final int? fresh = int.tryParse(match.group(2)!);
  if (fresh == null) {
    return null;
  }
  return (host: match.group(1)!, newFindings: fresh);
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

  /// Die Karte der zuletzt empfangenen `LLM_005`, oder null.
  ///
  /// Geführt unabhängig davon, was auf dem Schirm steht: Kam zuletzt eine
  /// Warnung für einen anderen Host, ist die Folge unterbrochen, auch wenn
  /// ihre Karte inzwischen ausgeblendet ist. Null nach einer `LLM_005`, deren
  /// Satz [llmWarningOf] nicht lesen kann (HUM-354).
  int? _lastLlmCard;

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
      final Diagnostic clean = sanitizeDiagnostic(diagnostic);
      final LlmWarning? warning = llmWarningOf(clean);
      final List<SessionDiagnostic> next = <SessionDiagnostic>[...state];
      // Eingereiht wird nur in die Karte der unmittelbar vorigen `LLM_005`,
      // und nur, solange sie offen ist und denselben Host nennt. Eine
      // ausgeblendete oder vom Deckel verdrängte Karte nimmt nichts mehr auf.
      final int group = warning == null || _lastLlmCard == null
          ? -1
          : next.indexWhere(
              (SessionDiagnostic e) =>
                  e.id == _lastLlmCard && e.llmHost == warning.host,
            );
      if (warning != null && group >= 0) {
        // Dieselbe Karte, eine Anfrage mehr: Id bleibt, damit „Ausblenden"
        // die ganze Gruppe trifft und nichts neu einblendet; Satz, Zeit und
        // Fluss kommen von der jüngsten, damit „Anfrage öffnen" sie öffnet.
        final SessionDiagnostic open = next.removeAt(group);
        next.add(
          SessionDiagnostic(
            id: open.id,
            at: at,
            flowId: flowId,
            diagnostic: clean,
            requests: open.requests + 1,
            newFindings: (open.newFindings ?? 0) + warning.newFindings,
            llmHost: warning.host,
          ),
        );
      } else {
        next.add(
          SessionDiagnostic(
            id: _next++,
            at: at,
            flowId: flowId,
            diagnostic: clean,
            newFindings: warning?.newFindings,
            llmHost: warning?.host,
          ),
        );
      }
      if (clean.code == 'LLM_005') {
        _lastLlmCard = warning == null ? null : next.last.id;
      }
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
  /// neue Karte; bei einer zusammengefassten `LLM_005`-Karte geht die ganze
  /// Gruppe, und die nächste Warnung beginnt eine neue.
  void dismiss(int id) {
    state = <SessionDiagnostic>[
      for (final SessionDiagnostic open in state)
        if (open.id != id) open,
    ];
  }
}
