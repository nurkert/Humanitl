/// Der Schnappschuss: die Abschnitte der Shell, während die Verbindung, die
/// stand, gebrochen ist (HUM-044, `docs/UX.md` 4.2, Fall 4).
///
/// Die Shell bleibt stehen, aber nichts darin darf sich weiter so verhalten,
/// als käme es gerade vom Daemon. Vier Dinge halten das ein:
///
/// - **Die Uhr steht.** Die Countdowns der Warteschlange lesen `nowProvider`,
///   die eine Uhr des Programms. Unter diesem Widget bekommt sie einen Ersatz,
///   der den Stand im Augenblick des Bruchs festhält und nie einen zweiten
///   veröffentlicht, so dass „wird in 1:47 blockiert" nicht weiterläuft,
///   obwohl niemand mehr da ist, der blockieren könnte. Der Ersatz gilt nur
///   hier: Ohne Bruch steht dieses Widget nicht im Baum, und die Warteschlange
///   liest die gewohnte Uhr.
///
///   Der Ersatz erreicht **jede** Anzeige, die ihre Sekunde von dort holt,
///   und nur die. Die Laufzeit der Sandbox gehört seit HUM-044 dazu; ein
///   Widget, das sich statt dessen einen eigenen `Timer.periodic` nimmt,
///   überlebt den Bruch, weil die Abschnitte umgehängt und nicht zerstört
///   werden, und zählt in einem Bild weiter, das als Schnappschuss
///   beschriftet ist. Deshalb steht die Uhr in `core/time/now.dart`, wo jedes
///   Feature sie lesen darf.
///
///   Der Stand kommt aus `queueClockProvider` und nicht aus einem eigenen
///   Blick auf die Uhr, damit es einen einzigen Augenblick des Bruchs gibt.
///   Die Zeile im Schnappschuss und der Countdown darin müssen von derselben
///   Sekunde reden, und seit die fünf Daten-Abschnitte je einen eigenen
///   Schnappschuss tragen (der Setup-Bildschirm bleibt bedienbar), gäbe es
///   sonst fünf Augenblicke statt einem.
///
///   Gelesen und nicht beobachtet: Ein `watch` auf `queueClockProvider` aus
///   dem Ersatz heraus wäre ein Kreis, denn dieser Provider beobachtet
///   seinerseits `nowProvider`, und das ist genau der, der hier überschrieben
///   wird.
/// - **Die Entscheidungen sind still.** `AbsorbPointer` nimmt der Fläche die
///   Zeiger, `ExcludeFocus` nimmt ihr die Tastatur; ein Klick oder ein `a`
///   ginge sonst in einen `Decide`-Aufruf, der nirgends ankommt.
/// - **Die Zeilen bleiben, wie sie sind.** Das steht nicht hier, sondern in
///   `flows.dart`: Ein Widget kann keine Ereignisse aufhalten, und ein
///   abgeleiteter Provider ohne `dependencies` wird im Wurzelbehälter gebaut
///   und läse den Ersatz oben gar nicht. Wer nur diese Fläche einfriert,
///   friert die Uhr ein und sonst nichts.
/// - **Nichts bewegt sich.** `TickerMode` hält die laufenden Animationen an,
///   damit der Schnappschuss auch einer ist.
///
/// # Was ein Bruch nicht wegnehmen darf: den Zustand der Abschnitte
///
/// Dieses Widget kommt bei einem Bruch in den Baum und geht beim
/// Wiederanschluss wieder heraus, und damit ändert sich an jeder Stelle des
/// `IndexedStack` der Widget-**Typ**. `Widget.canUpdate` sagt dann nein, und
/// ohne Gegenmaßnahme würde jedes Element darunter weggeworfen und neu
/// gebaut -- mit ihm jedes `State`. Genau dort wohnt aber, was
/// `docs/UX.md` 7 dorthin gelegt hat: die eingefrorene Reihenfolge der
/// Warteschlange, die zugelassenen Zeilen, die Zeigeranwesenheit, die
/// Staffelung der Ankünfte, die Scrollposition und jedes offene
/// `HCollapsible`. Der Zähler der ausstehenden Ankünfte ist dagegen ein
/// Provider und überlebt -- also stünde nach jedem Bruch eine Pille
/// „+6 neu" über sechs Zeilen, die das Pane gerade selbst neu zugelassen hat,
/// und ein Klick darauf täte nichts, weil die Warteliste im neuen `State`
/// leer ist (`docs/UX.md` 2.8 und 5.3).
///
/// Die Gegenmaßnahme steht in `ShellScreen._sections`: Jeder Abschnitt trägt
/// einen `GlobalKey`, und der hängt sein Element um, statt es zu zerstören.
/// Was dabei doch neu läuft, ist `didChangeDependencies`, also findet jeder
/// Provider nach dem Umhängen wieder den richtigen Bereich.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';

import '../../../core/time/now.dart';
import '../../intercept/providers/flows.dart';

/// Die eingefrorene Fläche.
class FrozenSections extends ConsumerStatefulWidget {
  /// Friert [child] ein.
  const FrozenSections({required this.child, super.key});

  /// Die Abschnitte der Shell.
  final Widget child;

  @override
  ConsumerState<FrozenSections> createState() => _FrozenSectionsState();
}

class _FrozenSectionsState extends ConsumerState<FrozenSections> {
  /// Der Augenblick, in dem die Verbindung wegging.
  ///
  /// Er kommt aus `queueClockProvider`, der genau dann stehen bleibt, wenn
  /// `linkLiveProvider` nein sagt -- also aus derselben Quelle, nach der sich
  /// die Warteschlange räumt. Gelesen wird er **über** dem eigenen Bereich,
  /// denn `ref` dieses Zustands gehört zum Behälter oberhalb der
  /// [ProviderScope] unten, und genau einmal: Ein zweiter Blick beim nächsten
  /// Neubau setzte den Schnappschuss weiter, und die Countdowns liefen in
  /// Sprüngen doch wieder.
  late final DateTime _frozenAt = ref.read(queueClockProvider);

  late final Override _clock = nowProvider.overrideWith(
    () => _FrozenNow(_frozenAt),
  );

  @override
  Widget build(BuildContext context) => ProviderScope(
    overrides: <Override>[_clock],
    child: TickerMode(
      enabled: false,
      child: ExcludeFocus(child: AbsorbPointer(child: widget.child)),
    ),
  );
}

/// Die Uhr, die stehen bleibt.
///
/// Kein Timer: Was hier tickte, ließe die Countdowns weiterlaufen.
class _FrozenNow extends Now {
  /// Hält [_at] fest.
  _FrozenNow(this._at);

  final DateTime _at;

  @override
  DateTime build() => _at;
}
