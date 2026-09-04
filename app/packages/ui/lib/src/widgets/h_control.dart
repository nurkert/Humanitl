import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shad;

import '../theme/h_theme.dart';
import '../theme/shadcn_theme.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import 'h_animated_fill.dart';
import 'h_focus_ring.dart';

/// Die Füllung, die ein Control in [states] ohne Übergang trüge.
typedef HControlFill = Color Function(HTokens tokens, Set<WidgetState> states);

/// Der Stil, mit dem `Clickable` aus `shadcn_flutter` dieses Control zeichnet.
///
/// [fill] ist die Farbe dieses Frames — der Zwischenwert des Übergangs, den
/// [HAnimatedFill] rechnet. Wer sie in die Dekoration einsetzt, bekommt die
/// 120 ms aus [HMotion.press] statt der Vorgabe der Bibliothek.
typedef HControlStyle = shad.AbstractButtonStyle Function(
  HTokens tokens,
  Color fill,
);

/// Eine Taste, die dem Bildschirm gehört und nicht dem Control.
///
/// `Clickable` aus `shadcn_flutter` bindet unter jedem Control vier
/// Pfeiltasten auf `DirectionalFocusIntent`, und das innerste `Shortcuts`
/// gewinnt: solange eine Zeile oder ein Knopf den Fokus hält, erreichten
/// `ArrowUp` und `ArrowDown` die Intents des Bildschirms nicht mehr. Die
/// Warteschlange ist aber ein einziger Fokusstopp mit Pfeiltasten-Navigation
/// darin (`docs/UX.md` 5.2).
///
/// Dieser Intent hebt die Bindung auf, ohne eine eigene zu setzen: er hat
/// nirgends eine Action, `ShortcutManager.handleKeypress` findet keine und
/// antwortet `KeyEventResult.ignored`, und damit steigt die Taste zum
/// nächsten `Shortcuts` auf — zum Bildschirm, und wenn der sie nicht bindet,
/// weiter zu den Vorgaben von `WidgetsApp`, wo sie wieder den Fokus bewegt.
@immutable
class HPassThroughIntent extends Intent {
  /// Creates the intent.
  const HPassThroughIntent();
}

/// Die vier Pfeiltasten, an den Bildschirm durchgereicht.
///
/// Steht hier und nicht in jedem Control, damit `HControl` und `HRow`
/// dieselbe Menge binden. Die Schlüssel sind `LogicalKeySet`, weil `Clickable`
/// seine Tastenkarte so führt und ein Eintrag nur dann einen ihrer eigenen
/// überschreibt, wenn er ihm gleicht.
final Map<LogicalKeySet, Intent> hArrowsToScreen = <LogicalKeySet, Intent>{
  LogicalKeySet(LogicalKeyboardKey.arrowUp): const HPassThroughIntent(),
  LogicalKeySet(LogicalKeyboardKey.arrowDown): const HPassThroughIntent(),
  LogicalKeySet(LogicalKeyboardKey.arrowLeft): const HPassThroughIntent(),
  LogicalKeySet(LogicalKeyboardKey.arrowRight): const HPassThroughIntent(),
};

/// Ein Zustand, in dem ein Control gezeigt wird, ohne dass ihn jemand auslöst.
///
/// Für die Galerie und für Golden-Tests, die weder überfahren noch gedrückt
/// halten können. Produktivcode setzt ihn nie.
enum HControlPreview {
  /// Als läge der Zeiger auf dem Control.
  hovered,

  /// Als wäre es gedrückt.
  pressed,

  /// Als hätte es den Tastaturfokus.
  focused;

  /// Der Zustand der Bibliothek, den diese Vorschau vortäuscht.
  WidgetState get state => switch (this) {
    HControlPreview.hovered => WidgetState.hovered,
    HControlPreview.pressed => WidgetState.pressed,
    HControlPreview.focused => WidgetState.focused,
  };
}

/// Die Verhaltensschicht, auf der jedes anfassbare Control dieses Pakets steht.
///
/// Darunter liegt `Clickable` aus `shadcn_flutter`, die Schicht, aus der auch
/// deren `Button` gemacht ist: Zeiger, Tastatur, Cursor, die Zustandsmenge und
/// das Auftragen von Dekoration, Polsterung, Schrift und Icon-Thema. Der Stil
/// kommt weiterhin aus ihrem Stilsystem (`AbstractButtonStyle`), also aus
/// [HShadcnButtonStyle] und aus den `ButtonTheme`-Einträgen, die `HTheme`
/// veröffentlicht.
///
/// **Warum `Clickable` und nicht `Button`.** `Button` reicht keine `shortcuts`
/// durch, und ohne die lässt sich die Pfeiltasten-Bindung von `Clickable`
/// nicht aufheben — das innerste `Shortcuts` gewinnt, und der Bildschirm
/// bekäme `ArrowUp` und `ArrowDown` nie zu sehen (siehe
/// [HPassThroughIntent]). Was `Button` darüber hinaus tut, steht hier, und
/// zwar in derselben Form: die sechs Eigenschaften des Stils in
/// `WidgetStateProperty` gefasst, und [leading] in einer Reihe davor, deren
/// Inhalt gebunden ist. Das Binden ist keine Kosmetik: in einem blanken `Row`
/// mit `MainAxisSize.min` bekommt ein flexloses Kind waagerecht unbeschränkte
/// Constraints, die Beschriftung bricht dann nicht um, sondern läuft über —
/// gemessen an einem Knopf mit Glyph in einem 160 Pixel breiten Kasten um 378
/// Pixel.
///
/// Drei Dinge tut dieses Widget, die die Bibliothek nicht mitbringt und die
/// `docs/adr/0009-ui-stack.md` namentlich nennt:
///
/// 1. **Die Füllung behält ihre Dauer.** Der Übergang der Bibliothek läuft
///    über ihre Animationsprimitive, und die baut ihren Controller ohne
///    `animationBehavior`: sobald die Plattform `disableAnimations` meldet,
///    kollabiert jede Dauer auf fünf Prozent. Deshalb steht `Clickable` hier
///    auf `disableTransition: true`, und den Übergang rechnet [HAnimatedFill]
///    mit [HMotion.press] (`docs/UX.md` 2.2 und 2.10).
/// 2. **Der Druck ist sichtbar.** Die Vorgaben der Bibliothek kennen nur
///    `hovered` und `disabled`; ihre einzige Druck-Rückmeldung hängt an
///    `enableFeedback`, und der ist auf dem Linux-Desktop aus. Die Füllung,
///    die [fill] für `pressed` liefert, ist die Antwort darauf.
/// 3. **Der Fokusring ist unserer.** [HFocusRing] reserviert seinen Platz,
///    animiert nicht und hält über einer deckenden Füllung zwei Pixel Fläche
///    frei. Der Ring der Bibliothek zeichnet über die eigene Kante hinaus und
///    blendet ein; er bleibt deshalb abgeschaltet, trägt aber dieselben Maße,
///    wo eine Komponente ihn fest eingebaut hat ([HFocusRingMetrics]).
///
/// **Ein Zustandsspeicher, nicht zwei.** Der `WidgetStatesController` gehört
/// diesem Widget und wird an `Clickable` gereicht; dort schreiben Zeiger und
/// Druck hinein, hier der Fokusknoten. Gelesen wird nur er. Der Fokus kommt
/// vom Knoten, weil `Clickable` ihn über den Highlight-Modus von Flutter
/// meldet und der bis zum ersten Tasten- oder Mausereignis auf `touch` steht —
/// auf dem Desktop ist der Ring aber nie optional (`docs/UX.md` 6). Neu
/// gebaut wird nur, wenn sich die Menge ändert, die dieses Widget wirklich
/// zeichnet; `Clickable` schreibt `disabled` mitten im Aufbau in den
/// Controller, und dieser Wert steht hier ohnehin schon aus [enabled].
class HControl extends StatefulWidget {
  /// Creates a control.
  const HControl({
    required this.fill,
    required this.style,
    required this.builder,
    required this.onPressed,
    this.leading,
    this.leadingGap,
    this.enabled,
    this.focusNode,
    this.autofocus = false,
    this.preview,
    this.radius,
    this.ring = true,
    super.key,
  });

  /// Die Füllung je Zustand.
  final HControlFill fill;

  /// Der Stil, in den die Füllung dieses Frames eingesetzt wird.
  final HControlStyle style;

  /// Der Inhalt, gebaut aus dem Zustand und der Füllung dieses Frames.
  final Widget Function(
    BuildContext context,
    Set<WidgetState> states,
    Color fill,
  )
  builder;

  /// Wird bei Tipp, `Enter` und Leertaste gerufen. Null schaltet das Control
  /// ab, sofern [enabled] nichts anderes sagt.
  final VoidCallback? onPressed;

  /// Ein Glyph vor dem Inhalt.
  final Widget? leading;

  /// Der Abstand zwischen [leading] und dem Inhalt.
  ///
  /// Null heißt [HSpace.x2] — derselbe Wert, den die Bibliothek aus ihrer
  /// Dichte zöge (`density.baseGap`), und den `HTheme` dort einträgt. Ein
  /// Rückfall auf null wäre kein Abstand, sondern ein vergessener.
  final double? leadingGap;

  /// Übergeht die Ableitung „aktiv, wenn [onPressed] gesetzt ist".
  final bool? enabled;

  /// Ein von außen gehaltener Fokusknoten.
  final FocusNode? focusNode;

  /// Nimmt den Fokus, sobald das Control zum ersten Mal gebaut wird.
  final bool autofocus;

  /// Zeigt das Control in diesem Zustand, unabhängig von Zeiger und Fokus.
  final HControlPreview? preview;

  /// Der Eckenradius, an dem der Fokusring entlangläuft.
  final double? radius;

  /// Ob dieses Control überhaupt einen Fokusring zeigt.
  final bool ring;

  /// Ob das Control auf Eingaben reagiert.
  bool get isEnabled => enabled ?? (onPressed != null);

  @override
  State<HControl> createState() => _HControlState();
}

class _HControlState extends State<HControl> {
  /// Die drei Zustände, die dieses Widget aus dem Controller liest.
  ///
  /// `disabled` steht ausdrücklich nicht darin: er kommt aus
  /// [HControl.isEnabled], und `Clickable` trägt ihn mitten im Aufbau in den
  /// Controller nach. Käme er von dort, löste dieser Nachtrag ein `setState`
  /// während eines Aufbaus aus.
  static const Set<WidgetState> _interaction = <WidgetState>{
    WidgetState.hovered,
    WidgetState.pressed,
    WidgetState.focused,
  };

  final WidgetStatesController _states = WidgetStatesController();
  FocusNode? _owned;

  /// Der Knoten, an dem [_syncFocus] gerade hängt.
  ///
  /// Nicht `widget.focusNode`: fällt der von einem Knoten auf null, ist
  /// [_focus] schon der eigene, und ein Abmelden am alten plus ein Anmelden
  /// am neuen hinge den Hörer ein zweites Mal an denselben eigenen Knoten.
  FocusNode? _listening;
  Set<WidgetState> _painted = const <WidgetState>{};

  FocusNode get _focus =>
      widget.focusNode ?? (_owned ??= FocusNode(debugLabel: 'HControl'));

  @override
  void initState() {
    super.initState();
    // Vorbelegt, damit der Nachtrag in `Clickable.initState` nichts meldet.
    _states.update(WidgetState.disabled, !widget.isEnabled);
    _states.addListener(_redraw);
    _attachFocus();
    if (widget.autofocus) {
      // `Clickable` kennt kein `autofocus`; das Control holt sich den Fokus
      // nach dem ersten Frame und nur, wenn im Fokusbereich noch niemand
      // steht — die Bedeutung, die `autofocus` in Flutter hat.
      WidgetsBinding.instance.addPostFrameCallback((Duration _) {
        if (!mounted || !widget.isEnabled) {
          return;
        }
        final FocusScopeNode scope = FocusScope.of(context);
        if (scope.focusedChild == null) {
          _focus.requestFocus();
        }
      });
    }
  }

  @override
  void didUpdateWidget(HControl oldWidget) {
    super.didUpdateWidget(oldWidget);
    _attachFocus();
  }

  @override
  void dispose() {
    _states.removeListener(_redraw);
    _listening?.removeListener(_syncFocus);
    _states.dispose();
    _owned?.dispose();
    super.dispose();
  }

  /// Hängt [_syncFocus] an den Knoten, der gerade gilt, und liest ihn.
  ///
  /// Das Lesen gehört dazu: ein Wechsel des Knotens wechselt auch den
  /// Fokuszustand, und ohne den Nachlauf behielte der Ring den Wert des
  /// alten.
  void _attachFocus() {
    final FocusNode next = _focus;
    if (identical(_listening, next)) {
      return;
    }
    _listening?.removeListener(_syncFocus);
    _listening = next;
    next.addListener(_syncFocus);
    _syncFocus();
  }

  /// Trägt den Fokus des Knotens in den einen Zustandsspeicher.
  void _syncFocus() {
    if (mounted) {
      _states.update(WidgetState.focused, _focus.hasFocus);
    }
  }

  void _redraw() {
    if (!mounted || setEquals(_resolved, _painted)) {
      return;
    }
    setState(() {});
  }

  /// Die Zustandsmenge dieses Frames.
  Set<WidgetState> get _resolved {
    final HControlPreview? preview = widget.preview;
    final bool disabled = !widget.isEnabled;
    if (preview != null) {
      return <WidgetState>{preview.state, if (disabled) WidgetState.disabled};
    }
    return <WidgetState>{
      for (final WidgetState state in _states.value)
        if (_interaction.contains(state)) state,
      if (disabled) WidgetState.disabled,
    };
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Set<WidgetState> states = _resolved;
    _painted = states;
    final bool enabled = widget.isEnabled;
    final Color target = widget.fill(tokens, states);
    // Die ruhende Füllung entscheidet über den Abstand des Rings, nicht die
    // dieses Frames: sonst wüchse und schrumpfte der reservierte Platz mitten
    // im Übergang, und das Control verschöbe seine Nachbarn beim Überfahren.
    final Color resting = widget.fill(tokens, const <WidgetState>{});
    final bool focused = states.contains(WidgetState.focused);

    // Der Zustandsanbieter steht **immer** im Baum, auch mit leerer Menge:
    // verschwände er, wenn [HControl.preview] wegfällt, verlöre
    // [HAnimatedFill] darunter seinen Platz und mit ihm den laufenden
    // Übergang.
    final Widget control = shad.WidgetStatesProvider(
      states: <WidgetState>{if (widget.preview != null) widget.preview!.state},
      child: HAnimatedFill(
        color: target,
        builder: (BuildContext context, Color fill) {
          final shad.AbstractButtonStyle style = widget.style(tokens, fill);
          final Widget? leading = widget.leading;
          Widget content = widget.builder(context, states, fill);
          if (leading != null) {
            // Dieselbe Bindung wie in `Button`: die Reihe nimmt ihre
            // Eigenbreite, und der Inhalt bekommt den Rest als Grenze statt
            // unendlich viel Platz. Ohne sie läuft eine lange Beschriftung
            // neben einem Glyph aus dem Kasten heraus.
            content = IntrinsicWidth(
              child: IntrinsicHeight(
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.center,
                  children: <Widget>[
                    leading,
                    SizedBox(width: widget.leadingGap ?? HSpace.x2),
                    Expanded(child: content),
                  ],
                ),
              ),
            );
          }
          return shad.Clickable(
            statesController: _states,
            focusNode: _focus,
            enabled: enabled,
            onPressed: widget.onPressed,
            disableTransition: true,
            disableFocusOutline: true,
            focusOutline: false,
            // Aus: die Rückmeldung auf einen Druck ist eine Füllung und kein
            // Schrumpfen der Fläche, und eine Haptik hat der Desktop nicht.
            enableFeedback: false,
            shortcuts: hArrowsToScreen,
            decoration: WidgetStateProperty.resolveWith(
              (Set<WidgetState> states) => style.decoration(context, states),
            ),
            mouseCursor: WidgetStateProperty.resolveWith(
              (Set<WidgetState> states) => style.mouseCursor(context, states),
            ),
            padding: WidgetStateProperty.resolveWith(
              (Set<WidgetState> states) => style.padding(context, states),
            ),
            textStyle: WidgetStateProperty.resolveWith(
              (Set<WidgetState> states) => style.textStyle(context, states),
            ),
            iconTheme: WidgetStateProperty.resolveWith(
              (Set<WidgetState> states) => style.iconTheme(context, states),
            ),
            margin: WidgetStateProperty.resolveWith(
              (Set<WidgetState> states) => style.margin(context, states),
            ),
            child: content,
          );
        },
      ),
    );

    return HTheme.host(
      context,
      widget.ring
          ? HFocusRing(
              visible: focused && enabled,
              radius: widget.radius,
              over: resting,
              child: control,
            )
          : control,
    );
  }
}
