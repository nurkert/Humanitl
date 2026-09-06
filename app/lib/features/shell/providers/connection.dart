/// Der alte Ort des Verbindungszustands, als Weiterleitung.
///
/// `ConnectionStatus`, `connectionStateProvider`, `daemonInfoProvider` und
/// `linkLiveProvider` stehen seit HUM-044 in `core/ipc/connection.dart`: Die
/// Warteschlange muss dieselbe Frage stellen wie die Shell, und ein Feature
/// darf kein anderes importieren (ARCHITECTURE 5). Diese Datei bleibt, weil
/// ein Dutzend Aufrufer und Tests sie unter diesem Pfad kennen und ein
/// Umzug von Importen nichts an der Sache verbessert.
library;

export 'package:humanitl/core/ipc/connection.dart';
