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

import '../features/history/long_body.dart';

/// The flow the detail golden shows: the fourth recorded one, a POST that was
/// allowed and answered.
const FlowId goldenDetailFlow = FlowId('018f0004-0000-7000-8000-00000000000a');

/// Ein JSON-Rumpf mit einem Fund darin: der Baum und seine Markierung
/// (HUM-116). Zeile 28 des Szenarios, ein PATCH mit zwei Funden.
const FlowId goldenJsonFlow = FlowId('018f0004-0000-7000-8000-00000000001d');

/// Der Flow, an dem die Notiz hängt: von Hand geblockt, mit dem Satz an den
/// Agenten unter der Entscheidung (HUM-117). Zeile 7 des Szenarios.
const FlowId goldenNoteFlow = FlowId('018f0004-0000-7000-8000-000000000007');

/// Ein Rumpf, der kein Text ist: die Telemetrie als Protobuf, in der
/// Hex-Ansicht (HUM-116). Zeile 15 des Szenarios, freigegeben, ohne Fund.
const FlowId goldenHexFlow = FlowId('018f0004-0000-7000-8000-000000000010');

/// Bearbeitet freigegeben (HUM-047): Zeile 6 des Szenarios, die Form
/// `allowEdited` der Aufzeichnung.
const FlowId goldenEditedFlow = FlowId('018f0004-0000-7000-8000-000000000006');

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

/// The history screen in [tokens], over a recorded session of [count] flows,
/// or over [client] when a golden needs a session of its own.
Widget historyGolden({
  required HTokens tokens,
  int count = 60,
  HistoryQuery? query,
  FlowId? selected,
  TextScaler textScaler = TextScaler.noScaling,
  bool exportOpen = false,
  FakeDaemonClient? client,
}) => ProviderScope(
  overrides: <Override>[
    daemonClientProvider.overrideWithValue(
      client ?? FakeDaemonClient.history(count: count),
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

/// Die Sitzung der übrigen Goldens, mit dem langen JSON-Rumpf an
/// [goldenDetailFlow].
FakeDaemonClient _longBodyClient() {
  final FakeDaemonClient client = FakeDaemonClient.history(count: 60);
  recordLongJsonRequest(client, goldenDetailFlow);
  return client;
}

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

    // Der Kopf eines bearbeitet hinausgegangenen Flusses: der Chip „Edited“
    // neben dem Zustand, dazu der Reiter „Edited“ (HUM-047).
    goldenTest(
      'history_detail_edited_$name',
      fileName: 'history_detail_edited_$name',
      constraints: window,
      builder: () => historyGolden(tokens: tokens, selected: goldenEditedFlow),
    );

    goldenTest(
      'history_detail_note_$name',
      fileName: 'history_detail_note_$name',
      constraints: window,
      builder: () => historyGolden(tokens: tokens, selected: goldenNoteFlow),
    );

    goldenTest(
      'history_detail_body_json_$name',
      fileName: 'history_detail_body_json_$name',
      constraints: window,
      builder: () => historyGolden(tokens: tokens, selected: goldenJsonFlow),
    );

    goldenTest(
      'history_detail_body_hex_$name',
      fileName: 'history_detail_body_hex_$name',
      constraints: window,
      builder: () => historyGolden(tokens: tokens, selected: goldenHexFlow),
    );

    // Ein Rumpf mit dreizehn Baumzeilen im Standardfenster: Titel und
    // mindestens zehn Zeilen stehen ohne Scrollen da (HUM-153).
    goldenTest(
      'history_detail_body_tree_$name',
      fileName: 'history_detail_body_tree_$name',
      constraints: window,
      builder: () => historyGolden(
        tokens: tokens,
        selected: goldenDetailFlow,
        client: _longBodyClient(),
      ),
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
