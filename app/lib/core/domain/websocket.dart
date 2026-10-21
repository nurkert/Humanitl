/// Ob eine gehaltene Anfrage eine WebSocket-Verbindung öffnet (HUM-058,
/// Zeile 11).
///
/// Die Datei steht in `core`, weil vier Stellen dieselbe Antwort brauchen:
/// Karte und Aktionsleiste der Warteschlange, der Editor und die Freigabe aus
/// der Benachrichtigung in der Shell; ein Feature importiert kein anderes
/// (ARCHITECTURE 5).
///
/// Die Karte und die Aktionsleiste sagen es, weil eine Freigabe hier mehr
/// freigibt als eine Anfrage: Nach dem Wechsel des Protokolls laufen die
/// Nachrichten durch, aufgezeichnet, aber nicht mehr angehalten
/// (`BACKLOG.md` 9, Punkt 10).
///
/// # Ein Spiegel, keine Entscheidung
///
/// Die Prüfung ist die des Proxys, Zeichen für Zeichen:
/// `humanitl_proxy::connect::requested_upgrade`
/// (`daemon/crates/proxy/src/connect.rs`) liest die erste Kopfzeile
/// `Upgrade` als gültigen Kopfzeilenwert und vergleicht sie ohne Rücksicht auf
/// Groß- und Kleinschreibung mit `websocket`. Daraus entsteht
/// `RequestKey.upgrade`, gegen das eine Regel mit `upgrade: websocket` geprüft
/// wird (`pipeline.rs`, `handler.rs`). Das Schema zählt dort nicht: Der Proxy
/// vergibt `ws` und `wss` nie, und eine Anfrage ohne die Kopfzeile ist für ihn
/// kein Wechsel. Der Recorder zählt das Schema zusätzlich
/// (`humanitl_recorder::writer::upgrade_of`); maßgeblich für die Freigabe ist
/// der Proxy, also folgt die Oberfläche ihm.
///
/// Der Vertrag trägt das Ergebnis heute nicht in `FlowSummary`; bis er es tut,
/// liest die Oberfläche dieselbe Kopfzeile, die der Mensch in der Karte
/// darunter sieht. Entschieden wird damit nichts: Ob eine Regel trifft, sagt
/// allein der Daemon (ADR-018).
library;

import 'dart:convert';

import 'domain.dart';

/// True when a request with [headers] asks for a WebSocket.
bool requestOpensWebSocket(List<Header> headers) {
  for (final Header header in headers) {
    if (header.name.toLowerCase() != 'upgrade') {
      continue;
    }
    // Der Proxy liest nur einen gültigen Kopfzeilenwert (`to_str`); ein Wert
    // mit Bytes außerhalb von sichtbarem ASCII ist für ihn kein Wechsel.
    if (header.value.any((int byte) => byte > 0x7E || byte < 0x20)) {
      return false;
    }
    return ascii.decode(header.value).toLowerCase() == 'websocket';
  }
  return false;
}

/// True when [detail] says enough about its request to allow it at all.
///
/// Ein Detail ohne Anfrage oder mit einer Anfrage ohne Kopfzeilen ist kein
/// Beweis, dass kein Protokollwechsel verlangt wird (HUM-058, Review): Eine
/// HTTP/1.1-Anfrage trägt mindestens `Host`, und eine Anfrage ganz ohne
/// Kopfzeilen ist eine nachgebaute, etwa aus einer unvollständigen
/// Aufzeichnung, während die gehaltene `Upgrade: websocket` tragen kann.
/// Dann gilt das Detail als nicht verfügbar, und die Anfrage bleibt gehalten.
///
/// Seit HUM-250 nimmt der Daemon die Kopfzeilen einer noch nicht
/// entschiedenen Anfrage aus seiner Live-Registry, bevor er auf die
/// Aufzeichnung zurückfällt; eine gehaltene Anfrage, deren Aufzeichnung
/// unvollständig ist, ist damit wieder entscheidbar. Die Prüfung bleibt für
/// den Rest: einen Daemon älterer Fassung und eine Anfrage, die die Registry
/// nicht mehr kennt.
bool requestIsReadable(FlowDetail? detail) =>
    detail?.request?.headers.isNotEmpty ?? false;

/// True when a request with [detail] may be allowed on a path that shows no
/// warning: readable ([requestIsReadable]) and without an upgrade.
///
/// [cardSeen] is true where the decision was taken on the card of exactly
/// this request with its detail loaded; there the warning stood, and a
/// WebSocket may go once it was read (HUM-058, row 11). Every other path --
/// a group, a selection, "allow all", the notification -- passes false.
bool detailSufficesForAllow(FlowDetail? detail, {required bool cardSeen}) =>
    allowHoldOf(detail, cardSeen: cardSeen) == null;

/// Why a request stays held on a path that shows no warning.
enum AllowHold {
  /// The detail did not arrive, or not in time.
  missing,

  /// The detail arrived without its headers ([requestIsReadable]).
  unreadable,

  /// The request asks for a WebSocket ([requestOpensWebSocket]).
  upgrade,
}

/// Why a request with [detail] may not be allowed here, or null when it may;
/// [cardSeen] as in [detailSufficesForAllow]. The reason picks the sentence:
/// a request whose detail did not come is no WebSocket, and saying so would
/// send somebody looking for one (HUM-058, review).
AllowHold? allowHoldOf(FlowDetail? detail, {required bool cardSeen}) {
  if (detail == null) {
    return AllowHold.missing;
  }
  if (!requestIsReadable(detail)) {
    return AllowHold.unreadable;
  }
  if (!cardSeen && requestOpensWebSocket(detail.request!.headers)) {
    return AllowHold.upgrade;
  }
  return null;
}
