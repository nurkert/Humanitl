/// Eine stehende Uhr für Tests, die `nowProvider` überschreiben.
///
/// Geteilt, weil mehr als ein Bildschirm die eine UI-Uhr liest: die
/// Countdowns der Warteschlange und die Laufzeit der Sandbox. Ein Golden, das
/// die echte Uhr liest, zeigt an jedem Tag ein anderes Bild (HUM-148).
library;

import 'package:humanitl/core/time/now.dart';

/// Eine stehende Uhr: kein Timer, dafür ein Zeitpunkt, den der Test setzt.
class FixedNow extends Now {
  /// Startet bei [_at].
  FixedNow(this._at);

  DateTime _at;

  @override
  DateTime build() => _at;

  /// Setzt die Uhr auf [at].
  void moveTo(DateTime at) {
    _at = at;
    state = at;
  }
}
