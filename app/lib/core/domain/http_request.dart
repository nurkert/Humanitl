/// Vom Entwurf zur Anfrage auf der Leitung (HUM-047).
///
/// `EditedRequest` (in `http.dart`) ist der Wire-Typ; dies hier ist der eine
/// Weg dorthin. Er steht in `core/domain` und nicht im Editor-Feature, weil
/// zwei Dinge daran hängen, die niemand zweimal schreiben soll: die Liste der
/// Kopfzeilen, die der Daemon selbst setzt, und die Prüfungen, die
/// `apply_edit` auf der Gegenseite noch einmal macht.
///
/// # Warum die Prüfungen hier trotzdem stehen
///
/// Sie sind **keine** Zusicherung; die gibt allein der Daemon. Sie sind der
/// Grund, warum der Knopf grau wird, statt eine Anfrage loszuschicken, die mit
/// `EDIT_002` zurückkommt: Ein Mensch soll den Fehler dort lesen, wo er ihn
/// gemacht hat, und nicht als Fehlermeldung nach dem Senden (`docs/UX.md` 4.4).
library;

import 'dart:convert';

import 'http.dart';

/// Die Kopfzeilen, die der Daemon selbst setzt und die deshalb nicht
/// mitgeschickt werden.
///
/// Dieselbe Liste wie `DAEMON_OWNED_HEADERS` in
/// `daemon/crates/proxy/src/edit.rs`. Sie werden dort still verworfen; hier
/// gar nicht erst gesendet, damit die Leitung nicht trägt, was ohnehin
/// wegfällt. `host` ist dabei: Das Ziel steht in `EditedRequest.url`, und eine
/// zweite Stelle, an der es stünde, wäre eine, an der es abweichen könnte.
const Set<String> daemonOwnedHeaders = <String>{
  'host',
  'content-length',
  'transfer-encoding',
  'content-encoding',
  'expect',
};

/// Die längste Methode, die noch ein Token ist.
const int methodMaxLength = 16;

/// Was an einer bearbeiteten Anfrage nicht stimmt.
///
/// Die Reihenfolge ist die von `apply_edit`: erst die Methode, dann der Pfad.
/// Das Ziel fehlt hier, weil der Entwurf es gar nicht ändern kann — die Felder
/// sind gesperrt, und der Wert kommt unverändert aus der gehaltenen Anfrage.
enum EditedRequestProblem {
  /// Die Methode ist leer, zu lang oder kein Großbuchstaben-Token.
  method,

  /// Der Pfad beginnt nicht mit `/`, oder er trägt ein Leer- oder
  /// Steuerzeichen.
  path,
}

/// Prüft Methode und Pfad, wie `apply_edit` es tun wird.
///
/// Gibt `null` zurück, wenn nichts dagegen spricht.
EditedRequestProblem? checkEditedRequest({
  required String method,
  required String pathAndQuery,
}) {
  if (!isValidMethod(method)) {
    return EditedRequestProblem.method;
  }
  if (!isValidOriginPath(pathAndQuery)) {
    return EditedRequestProblem.path;
  }
  return null;
}

/// `^[A-Z]{1,16}$`.
bool isValidMethod(String method) {
  if (method.isEmpty || method.length > methodMaxLength) {
    return false;
  }
  for (final int unit in method.codeUnits) {
    if (unit < 0x41 || unit > 0x5A) {
      return false;
    }
  }
  return true;
}

/// Origin-Form: führender Schrägstrich, kein Leerzeichen, nur sichtbares
/// ASCII.
///
/// Alles andere gehört prozent-kodiert. Die Grenze ist dieselbe wie im Daemon
/// (`check_path`), damit ein Pfad, den dieses Feld annimmt, dort nicht
/// abgelehnt wird.
bool isValidOriginPath(String pathAndQuery) {
  if (!pathAndQuery.startsWith('/')) {
    return false;
  }
  for (final int unit in pathAndQuery.codeUnits) {
    if (unit <= 0x20 || unit > 0x7E) {
      return false;
    }
  }
  return true;
}

/// Baut die Anfrage, die auf die Leitung geht.
///
/// [body] ist der fertige Text; er wird als UTF-8 kodiert, denn
/// `content-length` ist die **Byte**-Länge und nicht die Zahl der
/// UTF-16-Einheiten, die `String.length` zählt. Genau daran ist schon einmal
/// eine Anfrage mit einem Umlaut gescheitert (`backlog/sprint-4.md`, HUM-047
/// Fallstricke). Gezählt wird sie im Daemon; hier reisen die Bytes.
EditedRequest buildEditedRequest({
  required String method,
  required String url,
  required Iterable<({String name, String value})> headers,
  required String body,
}) => EditedRequest(
  method: _methodOf(method),
  methodRaw: method,
  url: url,
  headers: <Header>[
    for (final ({String name, String value}) header in headers)
      if (!daemonOwnedHeaders.contains(header.name.toLowerCase()))
        Header(name: header.name, value: utf8.encode(header.value)),
  ],
  body: utf8.encode(body),
);

/// Das Enum zu einer Methode; alles Unbekannte wird [Method.other] und reist
/// zusätzlich als Rohwert.
Method _methodOf(String method) => switch (method.toUpperCase()) {
  'GET' => Method.get,
  'HEAD' => Method.head,
  'POST' => Method.post,
  'PUT' => Method.put,
  'PATCH' => Method.patch,
  'DELETE' => Method.delete,
  'OPTIONS' => Method.options,
  'CONNECT' => Method.connect,
  'TRACE' => Method.trace,
  _ => Method.other,
};
