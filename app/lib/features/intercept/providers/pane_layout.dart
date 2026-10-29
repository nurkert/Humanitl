/// Widths of the three intercept panes.
///
/// The ratios live in a provider rather than in the screen so that a drag
/// survives a section switch: the shell keeps every section built, but the
/// screen state is rebuilt whenever the widget is replaced.
///
/// Seit HUM-305 überstehen sie auch einen Neustart: Sie stehen in der
/// Zustandsdatei der Oberfläche (HUM-254) unter [interceptPaneSplit] und nicht
/// in `SharedPreferences`, wie die Spezifikation von HUM-020 es vorsah; dieses
/// Paket ist keine Abhängigkeit der App, und die Datei gibt es schon.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/ui/pane_split.dart';
import '../../../core/ui/ui.dart';

/// The key the ratios are stored under in the UI state file.
const String paneRatiosSettingsKey = 'paneSplit.intercept';

/// The Intercept screen's panes: queue, inspector and domain pane, by default
/// in the proportions of [HSize.paneRatio].
final PaneSplitSpec interceptPaneSplit = PaneSplitSpec(
  paneRatiosSettingsKey,
  _defaultRatios(),
);

List<double> _defaultRatios() {
  final (int queue, int inspector, int domain) = HSize.paneRatio;
  final double total = (queue + inspector + domain).toDouble();
  return <double>[queue / total, inspector / total, domain / total];
}

/// Relative widths of queue, inspector and domain pane; they sum to one.
final NotifierProvider<PaneSplit, List<double>> paneRatiosProvider =
    paneSplitProvider(interceptPaneSplit);
