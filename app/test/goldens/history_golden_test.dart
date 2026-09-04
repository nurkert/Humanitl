// Goldens des History-Screens (HUM-032): die Tabelle, der Detailbereich mit
// der Anfrage und der abgelehnte Filter, je dunkel und hell. Erneuern mit
// `flutter test --update-goldens test/goldens`.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_screen.dart';
import 'package:humanitl/features/history/providers/history_detail.dart';
import 'package:humanitl/features/history/providers/history_query.dart';
import 'package:humanitl/l10n/l10n.dart';

/// The flow the detail golden shows: the fourth recorded one, a POST that was
/// allowed and answered.
const FlowId goldenDetailFlow = FlowId('018f0004-0000-7000-8000-00000000000a');

/// A query that does not change.
class FixedQuery extends HistoryQueryNotifier {
  /// Stays on [query].
  FixedQuery(this.query);

  /// What the bar shows.
  final HistoryQuery query;

  @override
  HistoryQuery build() => query;
}

/// A selection that does not change.
class FixedSelection extends HistorySelectionNotifier {
  /// Stays on [id].
  FixedSelection(this.id);

  /// The selected row.
  final FlowId? id;

  @override
  FlowId? build() => id;
}

/// The history screen in [tokens], over a recorded session of [count] flows.
Widget historyGolden({
  required HTokens tokens,
  int count = 60,
  HistoryQuery? query,
  FlowId? selected,
  TextScaler textScaler = TextScaler.noScaling,
  bool exportOpen = false,
}) => ProviderScope(
  overrides: <Override>[
    daemonClientProvider.overrideWithValue(
      FakeDaemonClient.history(count: count),
    ),
    if (query != null)
      historyQueryProvider.overrideWith(() => FixedQuery(query)),
    if (selected != null)
      historySelectionProvider.overrideWith(() => FixedSelection(selected)),
  ],
  child: WidgetsApp(
    color: tokens.colors.bg0,
    debugShowCheckedModeBanner: false,
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    // Like `app.dart`: no navigator, one overlay for everything that floats.
    builder: (BuildContext context, Widget? _) => MediaQuery(
      data: MediaQuery.of(context).copyWith(textScaler: textScaler),
      child: HTheme(
        tokens: tokens,
        child: ColoredBox(
          color: tokens.colors.bg0,
          child: Overlay(
            initialEntries: <OverlayEntry>[
              OverlayEntry(
                builder: (BuildContext context) =>
                    HistoryScreen(exportOpen: exportOpen),
              ),
            ],
          ),
        ),
      ),
    ),
  ),
);

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1400,
    height: 900,
  );

  for (final (String name, HTokens tokens) in <(String, HTokens)>[
    ('dark', HTokens.dark),
    ('light', HTokens.light),
  ]) {
    goldenTest(
      'history_table_$name',
      fileName: 'history_table_$name',
      constraints: window,
      builder: () => historyGolden(tokens: tokens),
    );

    goldenTest(
      'history_detail_request_$name',
      fileName: 'history_detail_request_$name',
      constraints: window,
      builder: () => historyGolden(tokens: tokens, selected: goldenDetailFlow),
    );

    goldenTest(
      'history_table_scale2_$name',
      fileName: 'history_table_scale2_$name',
      constraints: window,
      builder: () => historyGolden(
        tokens: tokens,
        count: 24,
        // `docs/UX.md` 6: up to twice the text size without an overflow and
        // without a cut-off line. The row heights grow with the scale.
        textScaler: const TextScaler.linear(2),
      ),
    );

    goldenTest(
      'history_export_$name',
      fileName: 'history_export_$name',
      constraints: window,
      builder: () => historyGolden(tokens: tokens, count: 24, exportOpen: true),
    );

    goldenTest(
      'history_filter_error_$name',
      fileName: 'history_filter_error_$name',
      constraints: window,
      builder: () => historyGolden(
        tokens: tokens,
        query: const HistoryQuery(filter: 'hosst:github.com'),
      ),
    );
  }
}
