/// How a screen splits its panes, remembered across restarts (HUM-305).
///
/// Every boundary between two panes can be dragged (`HResizablePanes`), and a
/// split somebody chose once should still be there the next morning: the
/// terminal that was dragged large stays large. The split lives in the UI
/// state file of HUM-254 under a key per screen, beside the width of the rail
/// and the dismissed notices. It is a convenience, and it fails like one: a
/// missing, unreadable or malformed entry is the default split, and a write
/// that does not go through leaves the split in memory for this run.
///
/// Während eines Zugs ändert sich nur der Speicher; geschrieben wird einmal,
/// wenn der Zug vorbei ist ([PaneSplit.commit]). Ein Zug liefert ein Ereignis
/// je Pixel, und eine Datei, die so oft geschrieben wird, wäre für nichts
/// geschrieben.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
// `NotifierProviderFamily` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart' show NotifierProviderFamily;

import '../ui_state/ui_state_file.dart';

/// One screen's panes: the key it is stored under and its default split.
///
/// Two specs are the same family member when their [key] is the same; the
/// defaults belong to the key and are not compared.
class PaneSplitSpec {
  /// Creates the spec of a screen.
  const PaneSplitSpec(this.key, this.defaults);

  /// The key in the UI state file, `paneSplit.<screen>`.
  final String key;

  /// The split a screen starts with and a double click restores, one ratio
  /// per pane, summing to one.
  final List<double> defaults;

  @override
  bool operator ==(Object other) => other is PaneSplitSpec && other.key == key;

  @override
  int get hashCode => key.hashCode;
}

/// The split of one screen's panes.
final NotifierProviderFamily<PaneSplit, List<double>, PaneSplitSpec>
paneSplitProvider =
    NotifierProvider.family<PaneSplit, List<double>, PaneSplitSpec>(
      PaneSplit.new,
    );

/// The notifier behind [paneSplitProvider].
class PaneSplit extends Notifier<List<double>> {
  /// Creates the notifier of [spec].
  PaneSplit(this.spec);

  /// The screen this split belongs to.
  final PaneSplitSpec spec;

  @override
  List<double> build() =>
      _valid(ref.read(uiStateFileProvider).read()[spec.key]) ?? spec.defaults;

  /// The ratios stored under the key, or null when they are not a split of
  /// this screen: not a list, the wrong number of panes, or a ratio that is
  /// not a positive finite number.
  List<double>? _valid(Object? stored) {
    if (stored is! List<Object?> || stored.length != spec.defaults.length) {
      return null;
    }
    final List<double> ratios = <double>[];
    for (final Object? value in stored) {
      if (value is! num || !value.isFinite || value <= 0) {
        return null;
      }
      ratios.add(value.toDouble());
    }
    return _normalised(ratios);
  }

  /// [ratios] scaled to a sum of one, or null when that gives no split: a
  /// sum that is not a positive finite number (`[1e308, 1e308]` overflows),
  /// or a pane that would end up with nothing.
  static List<double>? _normalised(List<double> ratios) {
    final double sum = ratios.fold(0, (double a, double b) => a + b);
    if (!sum.isFinite || sum <= 0) {
      return null;
    }
    final List<double> scaled = <double>[
      for (final double r in ratios) r / sum,
    ];
    return scaled.every((double r) => r.isFinite && r > 0) ? scaled : null;
  }

  /// Takes the ratios of a drag, normalised to a sum of one. Memory only;
  /// [commit] writes them down.
  void set(List<double> ratios) {
    if (ratios.length != spec.defaults.length) {
      return;
    }
    final List<double>? normalised = _normalised(ratios);
    if (normalised == null) {
      return;
    }
    state = normalised;
  }

  /// Writes the current split into the UI state file. Called when a drag
  /// ends and after a step of the keyboard.
  void commit() =>
      ref.read(uiStateFileProvider).setValue(spec.key, List<double>.of(state));

  /// Puts the default split back and writes it down: the double click on a
  /// boundary.
  void reset() {
    state = spec.defaults;
    commit();
  }
}
