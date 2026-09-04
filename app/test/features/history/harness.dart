// Der History-Screen allein, über einem FakeDaemonClient: Theme, Sprachen und
// ein Overlay wie in `app.dart`, aber ohne Shell. Die Shell baut jeden der
// fünf Screens gleichzeitig; ein Widget-Test der History soll nicht an einem
// anderen Screen scheitern.

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_screen.dart';
import 'package:humanitl/features/history/providers/history_page.dart';
import 'package:humanitl/l10n/l10n.dart';

/// The history screen under [client], in a window of [size].
///
/// Returns the container, so a test can read `navigationProvider` or the page
/// state without going through the tree.
Future<ProviderContainer> pumpHistory(
  WidgetTester tester, {
  required DaemonClient client,
  Size size = const Size(1400, 900),
  // Not a default value: `HTokens.dark` is no longer a compile-time constant.
  HTokens? tokens,
  Locale locale = const Locale('en'),
  TextScaler textScaler = TextScaler.noScaling,
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
      child: historyApp(
        tokens: tokens ?? HTokens.dark,
        locale: locale,
        textScaler: textScaler,
      ),
    ),
  );
  await settleHistory(tester, container);
  return container;
}

/// The widget tree around the screen; the golden test builds it directly.
Widget historyApp({
  HTokens? tokens,
  Locale locale = const Locale('en'),
  TextScaler textScaler = TextScaler.noScaling,
}) => _historyApp(tokens ?? HTokens.dark, locale, textScaler);

Widget _historyApp(HTokens tokens, Locale locale, TextScaler textScaler) =>
    WidgetsApp(
      color: HColors.bg0,
      debugShowCheckedModeBanner: false,
      locale: locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      onGenerateTitle: (BuildContext context) => 'Humanitl',
      builder: (BuildContext context, Widget? child) => MediaQuery(
        data: MediaQuery.of(context).copyWith(textScaler: textScaler),
        child: HTheme(
          tokens: tokens,
          child: ColoredBox(
            color: tokens.colors.bg0,
            child: Overlay(
              initialEntries: <OverlayEntry>[
                OverlayEntry(
                  builder: (BuildContext context) => const HistoryScreen(),
                ),
              ],
            ),
          ),
        ),
      ),
    );

/// Pumps until the first page has arrived.
Future<void> settleHistory(
  WidgetTester tester,
  ProviderContainer container,
) async {
  for (int i = 0; i < 60; i++) {
    await tester.pump(const Duration(milliseconds: 16));
    final HistoryPageState page = container.read(historyPageProvider);
    if (!page.loading && !page.loadingMore) {
      await tester.pump(const Duration(milliseconds: 500));
      return;
    }
  }
  fail('the history page never finished loading');
}
