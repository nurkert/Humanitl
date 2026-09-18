// Die Handler, über die eine Ausnahme der Dart-Seite ins Protokoll kommt
// (HUM-136).
//
// Der Test fährt den Weg, den ein echter Fehler nimmt: Ein Widget wirft beim
// Bauen, das Framework meldet das an `FlutterError.onError`, und danach muss
// in der Datei genau eine Zeile mehr stehen -- mit dem Fehlertext darin.

import 'dart:async';
import 'dart:io';
import 'dart:ui' show ErrorCallback, PlatformDispatcher;

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/diagnostics/app_log.dart';
import 'package:humanitl/core/diagnostics/error_handlers.dart';

void main() {
  late Directory home;
  late AppLog log;

  setUp(() {
    home = Directory.systemTemp.createTempSync('humanitl-hum136-');
    log = AppLog('${home.path}/humanitl/${AppLog.fileName}');
  });

  tearDown(() {
    if (home.existsSync()) {
      home.deleteSync(recursive: true);
    }
  });

  testWidgets('ein Fehler im Baum steht danach in der Datei', (
    WidgetTester tester,
  ) async {
    log.started(version: 'test');
    final int before = log.readLines().length;

    final ErrorHandlerRegistration registration = installErrorHandlers(log);
    addTearDown(registration.restore);

    await tester.pumpWidget(
      Builder(
        builder: (BuildContext context) {
          throw StateError('the terminal view is gone');
        },
      ),
    );

    expect(tester.takeException(), isStateError);

    final List<String> lines = log.readLines();
    expect(
      lines,
      hasLength(before + 1),
      reason: 'genau eine Zeile mehr, nicht eine je Bild',
    );
    expect(lines.last, contains('the terminal view is gone'));
    expect(lines.last, contains('source=${ErrorSource.flutterError}'));
  });

  testWidgets('der Handler, der vorher dort stand, läuft weiter', (
    WidgetTester tester,
  ) async {
    final List<Object> seen = <Object>[];
    final FlutterExceptionHandler? previous = FlutterError.onError;
    FlutterError.onError = (FlutterErrorDetails details) {
      seen.add(details.exception);
      previous?.call(details);
    };
    addTearDown(() => FlutterError.onError = previous);

    final ErrorHandlerRegistration registration = installErrorHandlers(log);
    addTearDown(registration.restore);

    FlutterError.reportError(
      FlutterErrorDetails(exception: StateError('chained')),
    );

    expect(tester.takeException(), isStateError);
    expect(
      seen.single,
      isStateError,
      reason: 'das Protokoll ersetzt keine bestehende Fehlerbehandlung',
    );
    expect(log.readLines().single, contains('chained'));
  });

  test('was die Zone fängt, trägt ihre Herkunft', () {
    runZonedGuarded(() {
      throw StateError('from the zone');
    }, zoneErrorHandler(log));

    final String line = log.readLines().single;
    expect(line, contains('from the zone'));
    expect(line, contains('source=${ErrorSource.zone}'));
  });

  test('die Registrierung gibt beide Handler zurück', () {
    final FlutterExceptionHandler? flutterBefore = FlutterError.onError;
    final ErrorCallback? platformBefore = PlatformDispatcher.instance.onError;

    final ErrorHandlerRegistration registration = installErrorHandlers(log);
    expect(FlutterError.onError, isNot(same(flutterBefore)));
    expect(PlatformDispatcher.instance.onError, isNot(same(platformBefore)));

    registration.restore();
    expect(FlutterError.onError, same(flutterBefore));
    expect(PlatformDispatcher.instance.onError, same(platformBefore));
  });

  test('ein Fehler der Plattform bleibt unbehandelt und steht trotzdem da', () {
    final ErrorHandlerRegistration registration = installErrorHandlers(log);
    addTearDown(registration.restore);

    final bool handled = PlatformDispatcher.instance.onError!(
      StateError('from the platform'),
      StackTrace.empty,
    );

    expect(
      handled,
      isFalse,
      reason: 'das Protokoll entscheidet nichts über den Fehler',
    );
    expect(log.readLines().single, contains('from the platform'));
    expect(
      log.readLines().single,
      contains('source=${ErrorSource.platformDispatcher}'),
    );
  });
}
