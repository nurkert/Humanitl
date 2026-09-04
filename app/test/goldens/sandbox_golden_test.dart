// Goldens des Sandbox-Screens (HUM-040): der Kopf in `stopped`, in `running`
// und in `failed`, je dunkel und hell. Erneuern mit
// `flutter test --update-goldens test/goldens`.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/ui.dart';

import '../features/sandbox/harness.dart';

void main() {
  // Nur der Kopf plus der Streifen darunter: die drei Zustände unterscheiden
  // sich genau dort, und ein volles Fenster machte aus drei Bildern drei
  // Bilder derselben Tabelle.
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1280,
    height: 800,
  );

  for (final (String name, HThemeMode mode) in <(String, HThemeMode)>[
    ('dark', HThemeMode.dark),
    ('light', HThemeMode.light),
  ]) {
    goldenTest(
      'sandbox_header_stopped_$name',
      fileName: 'sandbox_header_stopped_$name',
      constraints: window,
      pumpBeforeTest: _settle,
      builder: () => sandboxUnderTest(client: SandboxTestClient(), mode: mode),
    );

    goldenTest(
      'sandbox_header_running_$name',
      fileName: 'sandbox_header_running_$name',
      constraints: window,
      pumpBeforeTest: _settle,
      builder: () => sandboxUnderTest(client: runningClient(), mode: mode),
    );

    goldenTest(
      'sandbox_header_failed_$name',
      fileName: 'sandbox_header_failed_$name',
      constraints: window,
      pumpBeforeTest: _settle,
      builder: () => sandboxUnderTest(client: blockedClient(), mode: mode),
    );
  }
}

/// Ein Frame für die Anfrage, einer für die Antwort, dann die Wartefrist von
/// `HWait` und die Farbüberblendung des Zustandspunkts.
Future<void> _settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}
