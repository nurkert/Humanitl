// Wege durch das Einstellungs-Overlay (HUM-404) für die Tests, die eine
// Zeile außerhalb der ersten Kategorie brauchen.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/settings/registry/settings_registry.dart';
import 'package:humanitl/l10n/l10n.dart';

/// Die Kennungen der Kategorien, wie der Katalog des Daemons sie nennt
/// (HUM-534); der Test `daemon/crates/config/tests/catalog.rs` hält sie
/// dort fest. Nur für Tests: Die App liest ihre Kategorien aus dem Katalog.
abstract final class SettingsCategory {
  /// Die Modellserver.
  static const String models = 'models';

  /// Der Agent.
  static const String agent = 'agent';

  /// Das Agent-Depot (HUM-521).
  static const String agents = 'agents';

  /// Die Sandbox.
  static const String sandbox = 'sandbox';

  /// Die Anfragen.
  static const String requests = 'requests';

  /// Die Ansicht.
  static const String interface = 'interface';

  /// Die Tastenkürzel.
  static const String shortcuts = 'shortcuts';

  /// Das Netz.
  static const String network = 'network';

  /// Der Entscheidungs-Proxy.
  static const String egress = 'egress';

  /// Der Daemon.
  static const String daemon = 'daemon';

  /// Alle, in der Reihenfolge der linken Spalte.
  static const List<String> values = <String>[
    models,
    agent,
    agents,
    sandbox,
    requests,
    interface,
    shortcuts,
    network,
    egress,
    daemon,
  ];
}

/// Die Sicht des Overlays auf den Katalog des Fakes in [language], für einen
/// Daemon wie [info].
Future<SettingsView> fakeSettingsView({
  String language = 'en',
  DaemonInfo? info,
}) async {
  final SettingsCatalog catalog = await FakeDaemonClient(
    info: info ?? FakeDaemonClient.disposableInfo,
  ).getSettingsCatalog(language: language);
  return SettingsView(catalog, lookupAppLocalizations(Locale(language)));
}

/// Wählt [category] in der linken Spalte des offenen Overlays.
Future<void> showSettingsCategory(WidgetTester tester, String category) async {
  await tester.tap(find.byKey(ValueKey<String>('settings-nav-$category')));
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
