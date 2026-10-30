// Golden des Änderungs-Panels einer Wegwerf-Sandbox (HUM-351), dunkel und
// hell: die Liste des Szenarios `disposable` mit jeder Art, den Marken für
// Konflikt, Ablehnung und entfallene Sonderbits, und README.md aufgeklappt
// mit seinem Diff. Erneuern mit `make goldens-update`.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/ui.dart';

import '../features/sandbox/disposable_fixtures.dart';
import '../features/sandbox/harness.dart';

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1280,
    height: 800,
  );

  for (final (String name, HThemeMode mode) in <(String, HThemeMode)>[
    ('dark', HThemeMode.dark),
    ('light', HThemeMode.light),
  ]) {
    goldenTest(
      'sandbox_changes_panel_$name',
      fileName: 'sandbox_changes_panel_$name',
      constraints: window,
      pumpBeforeTest: _openWithDiff,
      builder: () => sandboxUnderTest(
        client: disposableClient(),
        mode: mode,
        locale: const Locale('en'),
      ),
    );
  }
}

/// Die erste Antwort, dann das Panel über den Kopf, dann README.md auf.
Future<void> _openWithDiff(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 900));
  await tester.tap(find.byKey(const Key('sandbox-changes-open')));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 900));
  await tester.tap(
    find.byKey(const ValueKey<String>('change-expand-$readmeId')),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 900));
}
