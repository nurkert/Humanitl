// Der Audit-Screen allein, über einem FakeDaemonClient: Theme, Sprachen und
// ein Overlay wie in `app.dart`, aber ohne Shell. Dieselbe Begründung wie in
// `test/features/history/harness.dart`: Die Shell baut alle Bildschirme
// zugleich, und ein Widget-Test des Audit-Screens soll nicht an einem anderen
// scheitern.

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/audit/audit_screen.dart';
import 'package:humanitl/features/audit/providers/audit_provider.dart';
import 'package:humanitl/l10n/l10n.dart';

/// Der Audit-Screen unter [client], in einem Fenster von [size].
///
/// Gibt den Container zurück, damit ein Test den Filter oder den Zustand der
/// Seite lesen kann, ohne durch den Baum zu gehen.
Future<ProviderContainer> pumpAudit(
  WidgetTester tester, {
  required DaemonClient client,
  Size size = const Size(1400, 900),
  HTokens? tokens,
  Locale locale = const Locale('en'),
  List<Override> overrides = const <Override>[],
}) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[
      daemonClientProvider.overrideWithValue(client),
      ...overrides,
    ],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: auditApp(tokens: tokens ?? HTokens.dark, locale: locale),
    ),
  );
  await settleAudit(tester, container);
  return container;
}

/// Der Baum um den Bildschirm.
Widget auditApp({HTokens? tokens, Locale locale = const Locale('en')}) =>
    _auditApp(tokens ?? HTokens.dark, locale);

Widget _auditApp(HTokens tokens, Locale locale) => WidgetsApp(
  color: HColors.bg0,
  debugShowCheckedModeBanner: false,
  locale: locale,
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  onGenerateTitle: (BuildContext context) => 'Humanitl',
  builder: (BuildContext context, Widget? child) => HTheme(
    tokens: tokens,
    child: ColoredBox(
      color: tokens.colors.bg0,
      child: Overlay(
        initialEntries: <OverlayEntry>[
          OverlayEntry(builder: (BuildContext context) => const AuditScreen()),
        ],
      ),
    ),
  ),
);

/// Pumpt, bis die erste Seite und die Prüfung angekommen sind.
Future<void> settleAudit(
  WidgetTester tester,
  ProviderContainer container,
) async {
  for (int i = 0; i < 60; i++) {
    await tester.pump(const Duration(milliseconds: 16));
    final AuditFilter filter = container.read(auditFilterProvider);
    final AuditRecordsState page = container.read(auditRecordsProvider(filter));
    final int run = container.read(auditRunProvider);
    final bool checked = !container.read(auditVerifyProvider(run)).isLoading;
    final bool headed = !container.read(auditHeadProvider(run)).isLoading;
    if (!page.loading && !page.loadingMore && checked && headed) {
      await tester.pump(const Duration(milliseconds: 500));
      return;
    }
  }
  fail('the audit screen never finished loading');
}
