import 'package:flutter/widgets.dart';

/// Durations and curves. Motion explains state changes, it does not decorate.
abstract final class HMotion {
  /// Entry easing. Not [Curves.easeOut]; do not substitute.
  static const Cubic enter = Cubic(0.2, 0, 0, 1);

  /// Exit easing.
  static const Cubic exit = Cubic(0.4, 0, 1, 1);

  /// A request arrives in the queue: 8 px slide plus fade.
  static const Duration arrive = Duration(milliseconds: 180);

  /// A button fills on press.
  static const Duration press = Duration(milliseconds: 120);

  /// The state rail sweeps after a decision.
  static const Duration sweep = Duration(milliseconds: 200);

  /// A decided row collapses and glides out.
  static const Duration leave = Duration(milliseconds: 220);

  /// A newly created rule draws itself in the rule list.
  static const Duration ruleDraw = Duration(milliseconds: 240);

  /// One breath of the countdown glyph below twenty percent.
  static const Duration breathe = Duration(milliseconds: 1200);

  /// Holding the left half of the release valve confirms after this long.
  static const Duration holdToConfirm = Duration(milliseconds: 400);

  /// Halten, das eine einzelne folgenreiche Entscheidung bestätigt.
  ///
  /// Kürzer als [holdToConfirm], weil hier nichts eingestellt wird: das Halten
  /// belegt nur, dass der Klick gewollt war. Gilt für Blockieren in der Zeile
  /// und in der Aktionsleiste; ab sechs Flows tritt an seine Stelle ein Modal.
  static const Duration holdToBlock = Duration(milliseconds: 250);

  /// Wie lange die URL eines neu ausgewählten Flows sichtbar sein muss, bevor
  /// Erlauben wieder feuert.
  ///
  /// Erlauben ist unumkehrbar, Enter ist eine einzelne Taste, und die Auswahl
  /// wandert nach jeder Entscheidung weiter. Ohne diese Frist erlaubt ein
  /// gedrückt gehaltenes Enter die halbe Queue ungelesen. Eine Sakkade auf
  /// eine neue Zeile plus eine Fixation dauert rund 300 ms; 350 ms ist damit
  /// die kleinste Frist, die ehrlich behaupten kann, die URL sei lesbar
  /// gewesen. Blockieren bleibt sofort verfügbar, weil der Agent es erneut
  /// versuchen darf.
  static const Duration rearm = Duration(milliseconds: 350);

  /// Versatz zwischen zwei Zeilen, die im selben Frame ankommen.
  ///
  /// Fünfzehn Anfragen, die gleichzeitig einblenden, liest das Auge als
  /// Flackern des Panes, nicht als fünfzehn ankommende Pakete.
  static const Duration stagger = Duration(milliseconds: 30);

  /// Wie viele Zeilen höchstens versetzt einfliegen.
  ///
  /// Alle weiteren fahren mit der letzten mit, damit ein Schwall nie länger
  /// als [staggerMax] mal [stagger] braucht, bis er vollständig steht.
  static const int staggerMax = 5;

  /// Anteil von [leave], in dem Gleiten und Ausblenden abgeschlossen sind.
  ///
  /// Die beiden Phasen des Abgangs überlappen: Gleiten und Ausblenden laufen
  /// in den ersten sechzig Prozent, das Zusammenfallen der Höhe in den letzten
  /// sechzig. Auf einer gemeinsamen Kurve steht die Zeile die halbe Zeit still
  /// und springt am Ende zur Seite; die Richtung, die die ganze Bedeutung
  /// trägt, ist dann unsichtbar.
  static const double leaveGlideFraction = 0.6;

  /// Wie lange der Bestätigungsstreifen einer Entscheidung stehen bleibt.
  ///
  /// Derselbe Wert wie das Fenster, in dem die entschiedene Zeile die Queue
  /// noch nicht verlassen hat: der Streifen gehört zu dieser Zeile.
  static const Duration confirm = Duration(seconds: 3);

  /// Wie lange „Regel gespeichert · Rückgängig" erreichbar bleibt.
  ///
  /// Danach verschwindet nur der Streifen; die Regel bleibt im Rules-Screen
  /// löschbar.
  static const Duration undoWindow = Duration(seconds: 10);

  /// Wie lange die Queue nach der letzten Tastaturnavigation eingefroren
  /// bleibt.
  static const Duration freezeAfterKey = Duration(seconds: 2);

  /// Wie lange die Queue eingefroren bleibt, nachdem der Zeiger sie verlassen
  /// hat.
  static const Duration freezeAfterPointer = Duration(milliseconds: 500);

  /// Die Auflösung, in der die Anwendung Zeit anzeigt.
  ///
  /// Countdown und Wartezeit stehen als `mm:ss` auf dem Schirm, also ist eine
  /// Sekunde die feinste Stufe, die ein Zuschauer unterscheiden kann. Was
  /// feiner sein muss — der Ring — bekommt einen eigenen Controller im Widget,
  /// nicht eine schnellere gemeinsame Uhr.
  static const Duration clockTick = Duration(seconds: 1);

  /// Ab welcher Wartezeit eine Oberfläche zugibt, dass sie wartet.
  ///
  /// Darunter bleibt alles stehen, wie es steht: eine Anzeige, die kürzer als
  /// eine Reaktionszeit sichtbar ist, wird als Flackern gelesen, nicht als
  /// Auskunft.
  static const Duration waitVisible = Duration(milliseconds: 150);

  /// Wie lange eine erschienene Warteanzeige mindestens stehen bleibt.
  ///
  /// Ohne Untergrenze erzeugt eine Antwort, die kurz nach [waitVisible]
  /// eintrifft, genau das Flackern, das [waitVisible] verhindern soll.
  static const Duration waitMinVisible = Duration(milliseconds: 400);

  /// Unterhalb welcher Restfrist der Countdown-Ring eigen animiert wird.
  ///
  /// Über einem Haltebudget von fünf Minuten wandert das Bogenende eines
  /// 16-px-Rings rund 0,15 px je Sekunde; ein eigener Controller je Zeile
  /// zeichnete dafür Subpixel neu. Erst unter einer Minute bewegt sich der
  /// Bogen rund 0,8 px je Sekunde, und die Sekundenschritte der gemeinsamen
  /// Uhr werden als Ruckeln sichtbar.
  static const Duration ringSmoothBelow = Duration(seconds: 60);

  /// Below this fraction of the hold budget the countdown glyph breathes.
  static const double breatheBelow = 0.2;

  /// Der zweite und letzte Schwellenwert des Atems.
  ///
  /// Zwei begrenzte Ereignisse sagen „jetzt hinsehen" und lassen den Menschen
  /// danach in Ruhe; ein endloser Puls im Augenwinkel nörgelt.
  static const double breatheBelowUrgent = 0.05;

  /// Wie viele Atemzüge ein erreichter Schwellenwert auslöst.
  static const int breatheCycles = 3;

  /// Die geringste Deckkraft, die der Atem dem Glyph erlaubt.
  ///
  /// Der Atem ist eine Flagge, keine Dringlichkeitsskala: die verbleibende
  /// Zeit steht in der Bogenlänge des Rings. Ein Glyph, das dabei fast
  /// verschwindet, macht die dringendste Anfrage zur am schlechtesten
  /// sichtbaren.
  static const double breatheMinOpacity = 0.72;

  /// Deckkraft des zweiten, ruhenden Rings, der den Atem unter reduzierter
  /// Bewegung ersetzt.
  ///
  /// Der Schwellenwert ist Information; ohne Ersatz verliert sie, wer
  /// Animationen abgeschaltet hat.
  static const double reducedRingAlpha = 0.4;

  /// Vertical offset of an arriving row.
  static const double arriveOffset = 8;

  /// Seitlicher Weg einer entschiedenen Zeile beim Verlassen.
  ///
  /// Das Anderthalbfache von [arriveOffset]: Gehen muss als längerer Weg
  /// lesbar sein als Kommen.
  static const double leaveOffset = arriveOffset * 1.5;
}

/// Löst Bewegung gegen die Systemeinstellung „Animationen reduzieren" auf.
///
/// Reduzierte Bewegung heißt weniger Weg, nicht weniger Rückmeldung. Strecken
/// und Schleifen laufen über diese Klasse und werden zu null; Dauern, die eine
/// Entscheidung bestätigen — Ausblenden, Tastenfüllung, Rail-Wisch —, bleiben
/// unverändert und lesen ihr Token direkt.
abstract final class HReducedMotion {
  /// Ob das System für [context] reduzierte Bewegung verlangt.
  static bool of(BuildContext context) =>
      MediaQuery.maybeDisableAnimationsOf(context) ?? false;

  /// [distance] in logischen Pixeln, unter reduzierter Bewegung null.
  static double distance(BuildContext context, double distance) =>
      of(context) ? 0 : distance;

  /// [duration] einer reinen Verschiebung, unter reduzierter Bewegung
  /// [Duration.zero].
  static Duration displace(BuildContext context, Duration duration) =>
      of(context) ? Duration.zero : duration;

  /// [cycles] Durchläufe einer Schleife, unter reduzierter Bewegung keiner.
  static int cycles(BuildContext context, int cycles) =>
      of(context) ? 0 : cycles;
}

/// Motion as instance data, reachable from `HTokens.motion`.
///
/// Nur die geführten Bewegungen stehen hier. Die Zeitfenster einer Regel —
/// [HMotion.confirm], [HMotion.undoWindow], [HMotion.freezeAfterKey],
/// [HMotion.freezeAfterPointer], [HMotion.clockTick] — sind Politik, keine
/// Animation, und bleiben statisch.
@immutable
class HMotionTokens {
  /// Creates a motion set. Use [standard].
  const HMotionTokens({
    this.enter = HMotion.enter,
    this.exit = HMotion.exit,
    this.arrive = HMotion.arrive,
    this.press = HMotion.press,
    this.sweep = HMotion.sweep,
    this.leave = HMotion.leave,
    this.ruleDraw = HMotion.ruleDraw,
    this.breathe = HMotion.breathe,
    this.holdToConfirm = HMotion.holdToConfirm,
    this.holdToBlock = HMotion.holdToBlock,
    this.stagger = HMotion.stagger,
  });

  /// The motion of the design direction.
  static const HMotionTokens standard = HMotionTokens();

  /// Entry easing.
  final Cubic enter;

  /// Exit easing.
  final Cubic exit;

  /// Arrival duration.
  final Duration arrive;

  /// Press duration.
  final Duration press;

  /// Rail sweep duration.
  final Duration sweep;

  /// Leave duration.
  final Duration leave;

  /// Rule draw duration.
  final Duration ruleDraw;

  /// Breathing period.
  final Duration breathe;

  /// Hold-to-confirm duration.
  final Duration holdToConfirm;

  /// Dauer des Haltens, das eine einzelne Blockierung bestätigt.
  final Duration holdToBlock;

  /// Versatz zwischen zwei gleichzeitig ankommenden Zeilen.
  final Duration stagger;
}
