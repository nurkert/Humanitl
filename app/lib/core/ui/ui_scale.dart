/// Die Skalierung der ganzen Anwendung (HUM-356).
///
/// Auf einem 4K-Schirm ohne Skalierung des Desktops ist alles zu klein. Die
/// Skalierung ändert deshalb die logische Pixeldichte des ganzen Baums: Bei
/// einem Faktor `s` legt sich die Anwendung in `Fenster / s` logischen Pixeln
/// an und malt mit `devicePixelRatio * s`. Jede Regel des Layouts sieht ein
/// kleineres logisches Fenster und bricht neu um; Text und Elemente wachsen
/// gemeinsam, und nichts wird als Bild vergrößert (`ui_scale_host.dart`).
///
/// Diese Datei hält die Zahlen und die Wahl: die Stufen der Einstellung, die
/// Schritte der Tasten, die Grenze, die aus der Größe des Fensters folgt, und
/// die gespeicherte Wahl selbst. Gerechnet wird in ganzen Prozent, damit
/// zehn Schritte nach oben und zehn nach unten wieder genau bei 100 landen.
///
/// Die Wahl überlebt einen Neustart in der kleinen Zustandsdatei der
/// Oberfläche (`UiStateFile`, HUM-254), gelesen einmal und synchron beim
/// ersten Bau wie die Schriftgröße des Terminals (`terminal_zoom.dart`):
/// asynchron gelesen legte sich das erste Bild bei 100 % an und spränge
/// einen Frame später, und das Terminal meldete dem Agenten zweimal eine
/// neue Größe.
library;

import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../shortcuts/intents.dart';
import '../ui_state/ui_state_file.dart';

/// Das kleinste Fenster, in das das Layout der drei Panes passt (HUM-019
/// Schritt 7), in logischen Pixeln bei 100 %.
///
/// Dasselbe Maß ist die Mindestgröße des nativen Fensters (`main.dart`,
/// `windowMinimumSize`) und die Grenze der Skalierung: Kein Faktor über
/// 100 % macht das logische Fenster kleiner als dieses Maß.
const Size appMinimumLayoutSize = Size(1100, 700);

/// Die Skalierung ohne Wahl, in Prozent.
const int uiScaleDefault = 100;

/// Die kleinste Skalierung, die die Tasten erreichen, in Prozent.
const int uiScaleMin = 50;

/// Die größte Skalierung, die die Tasten erreichen, in Prozent. Ob sie
/// gilt, entscheidet außerdem die Größe des Fensters ([uiScaleLimitFor]).
const int uiScaleMax = 300;

/// Wie viel ein Tastendruck ändert, in Prozent. Die Tasten rasten auf
/// Vielfachen davon ein: Von 125 geht es nach oben auf 130, nach unten auf
/// 120.
const int uiScaleStep = 10;

/// Die festen Stufen der Einstellung, in Prozent.
const List<int> uiScalePresets = <int>[90, 100, 110, 125, 150, 175, 200];

/// Der Schlüssel der Wahl in der Zustandsdatei.
const String uiScaleKey = 'uiScale';

/// Die größte Skalierung, bei der das logische Fenster noch mindestens
/// [appMinimumLayoutSize] misst, in ganzen Prozent und abgerundet.
///
/// Nie unter [uiScaleDefault]: Die Grenze nimmt nur Vergrößerung zurück. Ein
/// Fenster unter der Mindestgröße -- das native Fenster lässt es nicht zu,
/// ein Test schon -- bleibt bei der Wahl, statt verkleinert zu werden. Nie
/// über [uiScaleMax].
int uiScaleLimitFor(Size window) {
  if (!window.width.isFinite || !window.height.isFinite) {
    return uiScaleMax;
  }
  final double fit = math.min(
    window.width / appMinimumLayoutSize.width,
    window.height / appMinimumLayoutSize.height,
  );
  // Ein Hauch Toleranz: 1650 / 1100 ist im Gleitkomma nicht ganz 1,5.
  final int percent = (fit * 100 + 1e-6).floor();
  return percent.clamp(uiScaleDefault, uiScaleMax);
}

/// Die Skalierung, die gilt: die Wahl [preferred], über 100 % höchstens so
/// groß, wie das Fenster [window] trägt ([uiScaleLimitFor]).
///
/// Die Wahl selbst bleibt dabei stehen. Wird das Fenster wieder größer,
/// kehrt die gewählte Größe zurück; erst eine neue Wahl ändert sie.
int effectiveUiScale(int preferred, Size window) => preferred <= uiScaleDefault
    ? preferred
    : math.min(preferred, uiScaleLimitFor(window));

/// Die Wahl der Skalierung in Prozent, vor der Grenze des Fensters.
final NotifierProvider<UiScale, int> uiScaleProvider =
    NotifierProvider<UiScale, int>(UiScale.new);

/// Zählt jede Änderung, die jemand verlangt hat, auch eine, die an der
/// Grenze nichts mehr ändern konnte.
///
/// Der Wirt der Skalierung zeigt bei jedem neuen Wert kurz die Größe, die
/// jetzt gilt; ein Wechsel der Fenstergröße zählt nicht, weil ihn niemand
/// verlangt hat.
final NotifierProvider<UiScaleRequests, UiScaleRequest>
uiScaleRequestsProvider = NotifierProvider<UiScaleRequests, UiScaleRequest>(
  UiScaleRequests.new,
);

/// Die letzte verlangte Änderung: ihre laufende Nummer und ob das Fenster
/// sie abgewiesen hat.
@immutable
class UiScaleRequest {
  /// Die Anfrage [serial]; [refused], wenn ein Schritt nach oben an der
  /// Grenze des Fensters nichts ändern konnte.
  const UiScaleRequest(this.serial, {this.refused = false});

  /// Zählt jede Anfrage, damit auch zwei gleiche nacheinander ankommen.
  final int serial;

  /// Ob ein Schritt nach oben an der Grenze des Fensters stehen blieb.
  final bool refused;
}

/// Der Notifier hinter [uiScaleRequestsProvider].
class UiScaleRequests extends Notifier<UiScaleRequest> {
  @override
  UiScaleRequest build() => const UiScaleRequest(0);

  /// Eine Anfrage mehr; [refused], wenn das Fenster sie abwies.
  void count({bool refused = false}) =>
      state = UiScaleRequest(state.serial + 1, refused: refused);
}

/// Der Notifier hinter [uiScaleProvider].
class UiScale extends Notifier<int> {
  @override
  int build() {
    final Object? stored = ref.read(uiStateFileProvider).read()[uiScaleKey];
    if (stored is! num || !stored.isFinite) {
      return uiScaleDefault;
    }
    return stored.round().clamp(uiScaleMin, uiScaleMax);
  }

  /// Einen Schritt nach [step], innerhalb von [limit], der Grenze des
  /// Fensters in Prozent.
  ///
  /// Gerechnet wird von der Skalierung, die gilt, nicht von der Wahl: Steht
  /// die Wahl bei 200 und das Fenster trägt 154, macht `Ctrl+-` 150 daraus
  /// und nicht 190, was weiter 154 bliebe und wie ein toter Griff wirkte.
  /// Ein Schritt nach oben an der Grenze lässt die Wahl stehen, auch genau
  /// bei 100 % in einem Fenster der Mindestgröße: sonst stünde 110 % in der
  /// Datei und spränge an, sobald das Fenster wächst. Der Hinweis sagt dann,
  /// dass das Fenster nicht mehr trägt.
  void step(UiScaleStep step, {int limit = uiScaleMax}) {
    final int current = state <= uiScaleDefault
        ? state
        : math.min(state, limit);
    bool refused = false;
    switch (step) {
      case UiScaleStep.zoomIn:
        if (current >= limit) {
          // An [uiScaleMax] ist es die Grenze der Tasten, nicht des Fensters.
          refused = limit < uiScaleMax;
          break;
        }
        _set((current ~/ uiScaleStep + 1) * uiScaleStep);
      case UiScaleStep.zoomOut:
        _set((current - 1) ~/ uiScaleStep * uiScaleStep);
      case UiScaleStep.reset:
        _set(uiScaleDefault);
    }
    ref.read(uiScaleRequestsProvider.notifier).count(refused: refused);
  }

  /// Die Stufe [percent] aus der Einstellung.
  void choose(int percent) {
    _set(percent);
    ref.read(uiScaleRequestsProvider.notifier).count();
  }

  void _set(int percent) {
    final int next = percent.clamp(uiScaleMin, uiScaleMax);
    if (next == state) {
      return;
    }
    state = next;
    // Die Vorgabe wird als keine Wahl geschrieben, damit eine spätere
    // Vorgabe auch den erreicht, der zu ihr zurückgekehrt ist.
    ref
        .read(uiStateFileProvider)
        .setValue(uiScaleKey, next == uiScaleDefault ? null : next);
  }
}

/// Was der Wirt der Skalierung unter sich bekannt gibt: die Wahl, die
/// Grenze des Fensters und die Skalierung, die gilt.
///
/// Die Aktion der Tasten und die Einstellung brauchen die Grenze, um von der
/// geltenden Skalierung aus zu rechnen; sie kennt nur der Wirt, weil nur er
/// die wahre Größe des Fensters sieht. Unter ihm misst `MediaQuery` schon
/// das skalierte Fenster.
class UiScaleScope extends InheritedWidget {
  /// Gibt [preferred], [limit] und [effective] an [child] weiter.
  const UiScaleScope({
    required this.preferred,
    required this.limit,
    required this.effective,
    required super.child,
    super.key,
  });

  /// Die Wahl in Prozent.
  final int preferred;

  /// Die Grenze des Fensters in Prozent ([uiScaleLimitFor]).
  final int limit;

  /// Die Skalierung, die gilt, in Prozent ([effectiveUiScale]).
  final int effective;

  /// Der nächste Wirt über [context], oder null ohne einen (Tests, die eine
  /// Seite ohne die Anwendung bauen; dann gilt keine Grenze).
  static UiScaleScope? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<UiScaleScope>();

  /// Die Grenze des Fensters unter [context] zum Zeitpunkt des Aufrufs,
  /// ohne Abonnement: für Rückrufe wie einen Befehl der Palette, die später
  /// laufen als der Bau, der sie anlegte. Ohne Wirt [uiScaleMax].
  static int limitOf(BuildContext context) =>
      context.getInheritedWidgetOfExactType<UiScaleScope>()?.limit ??
      uiScaleMax;

  @override
  bool updateShouldNotify(UiScaleScope oldWidget) =>
      preferred != oldWidget.preferred ||
      limit != oldWidget.limit ||
      effective != oldWidget.effective;
}

/// Was die Shell an [UiScaleIntent] bindet (HUM-356).
///
/// Immer aktiv, in jedem Abschnitt und aus dem Terminal heraus: Die
/// Skalierung gehört der Anwendung, nicht dem Agenten (`docs/UX.md` 5.2).
class UiScaleAction extends ContextAction<UiScaleIntent> {
  /// Erzeugt die Aktion über [scale].
  UiScaleAction(this.scale);

  /// Der Notifier, den die Aktion ändert.
  final UiScale scale;

  @override
  Object? invoke(UiScaleIntent intent, [BuildContext? context]) {
    // Gelesen, nicht abonniert: Eine Aktion baut nichts.
    scale.step(
      intent.step,
      limit: context == null ? uiScaleMax : UiScaleScope.limitOf(context),
    );
    return null;
  }
}
