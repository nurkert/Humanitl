import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/colors.dart';
import '../tokens/flow_state.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_glyph.dart';
import 'h_hairline.dart';

/// The release valve: a split pill.
///
/// The left half performs the plain action once. Holding it for
/// [HMotion.holdToConfirm] fills green and performs the remembered variant. The
/// right half opens the duration and scope grid. The hairline between the two
/// halves is what makes it read as a valve rather than as one wide button.
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

  /// Colour of the fill and of the label; the allowed state colour when null.
  final Color? accent;

  /// Screen-reader label of the left half.
  final String? leftSemanticsLabel;

  /// Screen-reader label of the right half.
  final String? rightSemanticsLabel;

  @override
  State<HPill> createState() => _HPillState();
}

class _HPillState extends State<HPill> with SingleTickerProviderStateMixin {
  late final AnimationController _hold = AnimationController(
    vsync: this,
    duration: HMotion.holdToConfirm,
  );

  @override
  void dispose() {
    _hold.dispose();
    super.dispose();
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
    final Widget left = RawGestureDetector(
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
                        HColorDerivation.tint(accent),
                        HColorDerivation.tint(accent),
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
              style: tokens.typography.ui13.medium.tinted(accent),
              child: Center(widthFactor: 1, child: widget.left),
            ),
          ),
        ),
      ),
    );

    final Widget right = GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: widget.onRight,
      child: Semantics(
        button: true,
        label: widget.rightSemanticsLabel,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: HSpace.x2),
          child: Center(
            widthFactor: 1,
            child:
                widget.right ??
                HGlyphIcon(HGlyph.chevronRight, size: 14, color: accent),
          ),
        ),
      ),
    );

    return SizedBox(
      height: HSize.hitMin,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: HColorDerivation.tint(accent, 0.06),
          borderRadius: HRadius.controlRadius,
          border: Border.all(color: HColorDerivation.fade(accent, 0.4)),
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
