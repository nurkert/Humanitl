/// Was die Anwendung über ihren eigenen Lauf aufschreibt (HUM-136).
///
/// Nicht zu verwechseln mit dem `Diagnostic` des Daemons: Das ist ein Befund
/// über die Arbeit des Nutzers und steht in der Oberfläche. Hier geht es um
/// den Prozess selbst -- Start, Ende, Ausnahme, Signal -- und das Ziel ist
/// eine Datei, die nach einem verschwundenen Fenster einen Satz hergibt, mit
/// dem ein Mensch etwas anfangen kann.
library;

export 'app_log.dart';
export 'error_handlers.dart';
export 'redaction.dart';
