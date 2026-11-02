// Wege durch das Einstellungs-Overlay (HUM-404) für die Tests, die eine
// Zeile außerhalb der ersten Kategorie brauchen.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/settings/registry/setting_entry.dart';

/// Wählt [category] in der linken Spalte des offenen Overlays.
Future<void> showSettingsCategory(
  WidgetTester tester,
  SettingsCategory category,
) async {
  await tester.tap(
    find.byKey(ValueKey<String>('settings-nav-${category.name}')),
  );
  await tester.pumpAndSettle();
}

/// Tippt im Bedienelement [control] auf das Segment mit [label].
Future<void> tapSegment(WidgetTester tester, Key control, String label) async {
  final Finder segment = find.descendant(
    of: find.byKey(control),
    matching: find.text(label),
  );
  await tester.ensureVisible(segment);
  await tester.pumpAndSettle();
  await tester.tap(segment);
  await tester.pumpAndSettle();
}
