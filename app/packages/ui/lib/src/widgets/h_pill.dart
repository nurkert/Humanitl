import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/colors.dart';
import '../tokens/flow_state.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_focus_ring.dart';
import 'h_glyph.dart';
import 'h_hairline.dart';

/// The release valve: a split pill.
///
/// The left half performs the plain action once. Holding it for
/// [HMotion.holdToConfirm] fills green and performs the remembered variant. The
/// right half opens the duration and scope grid. The hairline between the two
/// halves is what makes it read as a valve rather than as one wide button.
///
/// Beide Hälften sind eigene Fokusstopps: jeder Zeigerweg hat eine Taste
/// (`docs/UX.md` 5.1 und 9, Punkt 17). Der Ring läuft als
/// [HFocusRing.inline] auf der Kante der Hälfte, weil außerhalb der Pille
/// kein Platz für ihn ist, ohne die Geometrie der Pille zu verschieben.
///
/// Fläche und Beschriftung werden getrennt geführt: [accent] ist die Fläche
/// und auf 3:1 geklemmt, das Wort, der Chevron und der Rahmen stehen in ihrer
/// Textvariante (`HTokens.stateTextOf`). Auf der eigenen Sechs-Prozent-
/// Füllung misst die Flächenfarbe hell 2,86:1 bis 3,36:1, auf der
/// Haltefüllung 2,49:1 bis 2,95:1 — das ist unter der 4,5:1 eines Wortes und
/// beim Chevron sogar unter den 3:1 einer Fläche (`docs/UX.md` 6). Der Rahmen
/// stand vorher bei [HColorDerivation.fade] auf 0,4 und maß gegen die eigene
/// Füllung 1,47:1; ein Rahmen, den man nicht sieht, macht aus der Pille
/// schwebenden Text.
///
/// Die Haltefüllung liegt **über** der Ruhefläche, also wird ihr Alpha
/// zurückgerechnet ([HColorDerivation.alphaOver]): 0,20 über 0,06 wären
/// wirksam 0,248, und für diese Fläche gilt keine der Zusicherungen der
/// Textableitung.
///
/// [onLeftLongPress] hat einen Tastenweg, und zwar denselben: `Enter` oder
/// die Leertaste **gehalten** füllt und bestätigt die gemerkte Variante,
/// kurz gedrückt löst [onLeft] aus. Ohne ihn wäre die gemerkte Handlung nur
/// mit der Maus erreichbar, und `docs/UX.md` 5.1 verlangt für Press-and-hold
/// ausdrücklich eine Taste. Eine Tastenwiederholung entscheidet nie (5.4).
class HPill extends StatefulWidget {
  /// Creates a split pill.
  const HPill({
    required this.left,
    required this.onLeft,
    this.right,
    this.onRight,
    this.onLeftLongPress,
    this.accent,
    this.leftSemanticsLabel,
    this.rightSemanticsLabel,
    super.key,
  });

  /// Content of the left, primary half.
  final Widget left;

  /// Content of the right half; a chevron when null.
  final Widget? right;

  /// Invoked when the left half is tapped.
  final VoidCallback? onLeft;

  /// Invoked when the right half is tapped.
  final VoidCallback? onRight;

  /// Invoked after holding the left half for [HMotion.holdToConfirm].
  final VoidCallback? onLeftLongPress;

  /// Die Farbe der Fläche; die Zustandsfarbe `allowed`, wenn null.
  ///
  /// Beschriftung und Chevron nehmen ihre Textvariante, nicht diese Farbe.
  final Color? accent;

  /// Screen-reader label of the left half.
  final String? leftSemanticsLabel;

  /// Screen-reader label of the right half.
  final String? rightSemanticsLabel;

  @override
  State<HPill> createState() => _HPillState();
}

class _HPillState extends State<HPill> with SingleTickerProviderStateMixin {
  // `AnimationBehavior.preserve`, und das ist der ganze Schutz: mit dem
  // normalen Verhalten skaliert Flutter jede Dauer auf fünf Prozent, sobald
  // die Plattform `disableAnimations` meldet — und der Linux-Embedder meldet
  // es wirklich. Aus 400 ms würden 20 ms, und ein gewöhnlicher Klick
  // bestätigte, was ein Halten bestätigen soll. Das Halten ist keine
  // schmückende Animation, es ist die Zeit, in der eine Entscheidung
  // zurückgenommen werden kann (`docs/UX.md` 2.10 und 5.4).
  late final AnimationController _hold = AnimationController(
    vsync: this,
    duration: HMotion.holdToConfirm,
    animationBehavior: AnimationBehavior.preserve,
  );

  bool _leftFocused = false;
  bool _rightFocused = false;

  /// Ob dieses Halten die gemerkte Variante schon ausgelöst hat.
  bool _keyFired = false;

  /// Die Frist des gehaltenen Tastendrucks.
  ///
  /// Eine eigene Uhr und nicht der Controller: der Zeigerweg misst seine Zeit
  /// im `LongPressGestureRecognizer`, also misst der Tastenweg sie genauso,
  /// und die Füllung bleibt in beiden Fällen die Anzeige und nicht der
  /// Entscheider.
  Timer? _keyHold;

  late final FocusNode _leftNode = FocusNode(
    debugLabel: 'HPill left',
    onKeyEvent: _onLeftKey,
  );

  @override
  void dispose() {
    _keyHold?.cancel();
    _hold.dispose();
    _leftNode.dispose();
    super.dispose();
  }

  static bool _activates(LogicalKeyboardKey key) =>
      key == LogicalKeyboardKey.enter ||
      key == LogicalKeyboardKey.numpadEnter ||
      key == LogicalKeyboardKey.space;

  KeyEventResult _onLeftKey(FocusNode node, KeyEvent event) {
    if (widget.onLeftLongPress == null || !_activates(event.logicalKey)) {
      return KeyEventResult.ignored;
    }
    if (event is KeyRepeatEvent) {
      // Eine gedrückt gehaltene Taste wiederholt; entscheiden darf sie nicht.
      return KeyEventResult.handled;
    }
    if (event is KeyDownEvent) {
      _keyFired = false;
      _keyHold?.cancel();
      _keyHold = Timer(HMotion.holdToConfirm, () {
        _keyFired = true;
        widget.onLeftLongPress?.call();
      });
      _hold.forward(from: 0);
      return KeyEventResult.handled;
    }
    if (event is KeyUpEvent) {
      final bool short = !_keyFired;
      _keyHold?.cancel();
      _keyHold = null;
      _hold.reverse();
      if (short) {
        widget.onLeft?.call();
      }
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  /// Macht [child] zu einem Fokusstopp, der [onActivate] auf `Enter` und
  /// `Leertaste` auslöst, und zeichnet den Ring auf seine Kante.
  Widget _focusable({
    required Widget child,
    required VoidCallback? onActivate,
    required bool focused,
    required ValueChanged<bool> onFocusChange,
    FocusNode? focusNode,
  }) {
    return FocusableActionDetector(
      enabled: onActivate != null,
      focusNode: focusNode,
      onFocusChange: onFocusChange,
      mouseCursor: onActivate == null
          ? MouseCursor.defer
          : SystemMouseCursors.click,
      actions: <Type, Action<Intent>>{
        ActivateIntent: CallbackAction<ActivateIntent>(
          onInvoke: (ActivateIntent intent) {
            onActivate?.call();
            return null;
          },
        ),
      },
      child: HFocusRing.inline(visible: focused, child: child),
    );
  }

  Map<Type, GestureRecognizerFactory> get _leftGestures {
    return <Type, GestureRecognizerFactory>{
      TapGestureRecognizer:
          GestureRecognizerFactoryWithHandlers<TapGestureRecognizer>(
            TapGestureRecognizer.new,
            (TapGestureRecognizer instance) {
              instance.onTap = widget.onLeft;
            },
          ),
      if (widget.onLeftLongPress != null)
        LongPressGestureRecognizer:
            GestureRecognizerFactoryWithHandlers<LongPressGestureRecognizer>(
              () => LongPressGestureRecognizer(duration: HMotion.holdToConfirm),
              (LongPressGestureRecognizer instance) {
                instance
                  ..onLongPressDown = (LongPressDownDetails details) {
                    _hold.forward();
                  }
                  ..onLongPressCancel = () {
                    _hold.reverse();
                  }
                  ..onLongPress = widget.onLeftLongPress
                  ..onLongPressEnd = (LongPressEndDetails details) {
                    _hold.reverse();
                  };
              },
            ),
    };
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color accent = widget.accent ?? tokens.state.allowed;
    // Die Fläche ist auf 3:1 geklemmt; ein Wort darauf braucht 4,5:1.
    final Color label = tokens.stateTextOf(accent);
    // Die Haltefüllung steht auf der Ruhefläche, nicht auf der nackten
    // Fläche des Panels; ihr Alpha wird deshalb zurückgerechnet, damit
    // zusammen genau [HColors.fillHoldAlpha] herauskommt.
    final Color holdFill = accent.withValues(
      alpha: HColorDerivation.alphaOver(
        HColors.fillHoldAlpha,
        HColors.fillRestAlpha,
      ),
    );
    final Widget leftHalf = RawGestureDetector(
      behavior: HitTestBehavior.opaque,
      gestures: _leftGestures,
      child: Semantics(
        button: true,
        label: widget.leftSemanticsLabel,
        child: AnimatedBuilder(
          animation: _hold,
          builder: (BuildContext context, Widget? child) => DecoratedBox(
            decoration: BoxDecoration(
              gradient: _hold.value == 0
                  ? null
                  : LinearGradient(
                      colors: <Color>[
                        holdFill,
                        holdFill,
                        const Color(0x00000000),
                        const Color(0x00000000),
                      ],
                      stops: <double>[0, _hold.value, _hold.value, 1],
                    ),
            ),
            child: child,
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: HSpace.x3),
            child: DefaultTextStyle(
              style: tokens.typography.ui13.medium.tinted(label),
              child: Center(
                widthFactor: 1,
                heightFactor: 1,
                child: widget.left,
              ),
            ),
          ),
        ),
      ),
    );

    final Widget left = _focusable(
      child: leftHalf,
      onActivate: widget.onLeft,
      focused: _leftFocused,
      focusNode: _leftNode,
      onFocusChange: (bool value) => setState(() => _leftFocused = value),
    );

    final Widget rightHalf = GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: widget.onRight,
      child: Semantics(
        button: true,
        label: widget.rightSemanticsLabel,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: HSpace.x2),
          child: Center(
            widthFactor: 1,
            heightFactor: 1,
            child:
                widget.right ??
                HGlyphIcon(HGlyph.chevronRight, size: 14, color: label),
          ),
        ),
      ),
    );

    final Widget right = _focusable(
      child: rightHalf,
      onActivate: widget.onRight,
      focused: _rightFocused,
      onFocusChange: (bool value) => setState(() => _rightFocused = value),
    );

    return ConstrainedBox(
      constraints: const BoxConstraints(minHeight: HSize.hitMin),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: HColorDerivation.tint(accent, HColors.fillRestAlpha),
          borderRadius: HRadius.controlRadius,
          // Derselbe Ton wie das Wort: der Rahmen ist die Kante des Controls
          // und erreicht damit 3:1 gegen die eigene Füllung und gegen jede
          // Fläche, auf der die Pille steht.
          border: Border.all(color: label),
        ),
        child: ClipRRect(
          borderRadius: HRadius.controlRadius,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              left,
              const HHairline(vertical: true, length: HSize.hitMin),
              right,
            ],
          ),
        ),
      ),
    );
  }
}
