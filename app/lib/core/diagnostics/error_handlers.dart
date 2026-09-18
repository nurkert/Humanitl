/// Die drei Stellen, an denen eine Ausnahme der Dart-Seite ankommt, und was
/// sie ins Protokoll schreiben (HUM-136).
///
/// Drei sind es, weil Flutter drei Wege kennt und keiner die anderen deckt:
/// `FlutterError.onError` für alles, was das Framework beim Bauen, Messen
/// oder Zeichnen fängt, `PlatformDispatcher.instance.onError` für das, was in
/// der Wurzel-Zone ankommt (etwa ein nicht behandelter Fehler eines Futures
/// der Plattform), und die Zone um `runApp` für den Rest.
///
/// Keiner der drei verschluckt etwas. Jeder schreibt seine Zeile und ruft
/// danach den Handler, der vorher dort stand -- in der Anwendung ist das
/// `FlutterError.presentError`, im Test die Aufzeichnung des Test-Bindings.
/// Ein Protokoll, das dafür die bestehende Fehlerbehandlung ersetzt, tauscht
/// eine Auskunft gegen eine andere.
///
/// Was hier **nicht** ankommt, ist der Grund, warum es `linux/runner` gibt:
/// `runZonedGuarded` fängt nichts, was im nativen Teil abstürzt, und gar
/// nichts, wenn der Prozess von außen beendet wird.
library;

import 'dart:ui' show ErrorCallback, PlatformDispatcher;

import 'package:flutter/foundation.dart';

import 'app_log.dart';

/// Woher eine Ausnahme kam. Steht als `source=` in der Zeile.
abstract final class ErrorSource {
  /// Das Framework hat sie gefangen: Bauen, Messen, Zeichnen, Gesten.
  static const String flutterError = 'FlutterError.onError';

  /// Die Wurzel-Zone der Plattform hat sie gemeldet.
  static const String platformDispatcher = 'PlatformDispatcher.onError';

  /// Die Zone um `runApp` hat sie gefangen.
  static const String zone = 'runZonedGuarded';
}

/// Die Handler, die vor [installErrorHandlers] standen.
///
/// Ein Test setzt sie über [restore] zurück; ohne das trüge der nächste Test
/// derselben Datei seine Fehler noch in das Protokoll des vorigen ein.
class ErrorHandlerRegistration {
  /// Merkt sich, was vorher dort stand.
  const ErrorHandlerRegistration(
    this._previousFlutterError,
    this._previousPlatformError,
  );

  final FlutterExceptionHandler? _previousFlutterError;
  final ErrorCallback? _previousPlatformError;

  /// Setzt beide Handler auf den Stand vor der Registrierung zurück.
  void restore() {
    FlutterError.onError = _previousFlutterError;
    PlatformDispatcher.instance.onError = _previousPlatformError;
  }
}

/// Der dritte Handler: was `runZonedGuarded` fängt.
///
/// Er ist eine eigene Funktion und kein Lambda in `main.dart`, weil sonst die
/// einzige Stelle, an der `ErrorSource.zone` entsteht, in einem Aufruf steckt,
/// den kein Test erreicht -- `main` startet die Anwendung.
void Function(Object, StackTrace) zoneErrorHandler(AppLog log) {
  return (Object error, StackTrace stack) {
    log.exception(error, stack: stack, source: ErrorSource.zone);
  };
}

/// Hängt [log] an `FlutterError.onError` und an
/// `PlatformDispatcher.instance.onError`.
///
/// Die dritte Stelle, die Zone, ist keine Registrierung, sondern ein
/// Aufrufparameter von `runZonedGuarded`; `main.dart` gibt dort
/// [ErrorSource.zone] mit.
ErrorHandlerRegistration installErrorHandlers(AppLog log) {
  final FlutterExceptionHandler? previousFlutterError = FlutterError.onError;
  final ErrorCallback? previousPlatformError =
      PlatformDispatcher.instance.onError;

  FlutterError.onError = (FlutterErrorDetails details) {
    log.exception(
      details.exception,
      stack: details.stack,
      source: ErrorSource.flutterError,
    );
    previousFlutterError?.call(details);
  };

  PlatformDispatcher.instance.onError = (Object error, StackTrace stack) {
    log.exception(error, stack: stack, source: ErrorSource.platformDispatcher);
    // `false` heißt „nicht behandelt": Der Fehler nimmt danach denselben Weg
    // wie ohne diesen Handler. Das Protokoll entscheidet nichts.
    return previousPlatformError?.call(error, stack) ?? false;
  };

  return ErrorHandlerRegistration(previousFlutterError, previousPlatformError);
}
