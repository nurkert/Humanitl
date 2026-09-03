/// Widths of the three intercept panes.
///
/// The ratios live in a provider rather than in the screen so that a drag
/// survives a section switch: the shell keeps every section built, but the
/// screen state is rebuilt whenever the widget is replaced.
///
/// They do **not** survive a restart yet. The specification asks for
/// `SharedPreferences` under the key `intercept.pane_ratios`; that package is
/// not a dependency of the app, and adding a plugin also regenerates the Linux
/// plugin registrant. Persisting the ratios is therefore left to the issue
/// that adds the settings storage; the notifier below is the single place that
/// has to learn how to read and write them.
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/ui/ui.dart';

part 'pane_layout.g.dart';

/// The key the ratios will be stored under once the app has a settings store.
const String paneRatiosSettingsKey = 'intercept.pane_ratios';

/// Relative widths of queue, inspector and domain pane; they sum to one.
@Riverpod(keepAlive: true)
class PaneRatios extends _$PaneRatios {
  @override
  List<double> build() {
    final (int queue, int inspector, int domain) = HSize.paneRatio;
    final double total = (queue + inspector + domain).toDouble();
    return <double>[queue / total, inspector / total, domain / total];
  }

  /// Takes the widths a drag produced, normalised to a sum of one.
  void set(List<double> ratios) {
    final double sum = ratios.fold(0, (double a, double b) => a + b);
    if (sum <= 0) {
      return;
    }
    state = <double>[for (final double r in ratios) r / sum];
  }
}
