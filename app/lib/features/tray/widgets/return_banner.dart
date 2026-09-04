/// The return banner: what happened while the window was away (HUM-034).
///
/// The return plays nothing back. Fifteen arrivals that happened minutes ago
/// get no arrival animation, because an animation answers "what has just
/// arrived" and none of this just arrived (`docs/UX.md` 4.9). What is left is
/// one quiet line that names the longest wait and leads to the request behind
/// it.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// The banner.
class ReturnBanner extends StatelessWidget {
  /// Creates a banner that says [sentence].
  const ReturnBanner({
    required this.sentence,
    required this.onJump,
    required this.onDismiss,
    super.key,
  });

  /// The one line: how long the agent has been waiting.
  final String sentence;

  /// Selects the request that has waited longest and closes the banner.
  final VoidCallback onJump;

  /// Closes the banner and does nothing else.
  final VoidCallback onDismiss;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return ArriveFromTop(
      child: Semantics(
        container: true,
        label: sentence,
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: HColorDerivation.tint(tokens.state.held),
            border: Border(bottom: BorderSide(color: tokens.colors.line)),
          ),
          child: ConstrainedBox(
            // A minimum, not a height: at `TextScaler` 2.0 the line is taller
            // than 36 px and a fixed box would swallow the overflow in
            // silence (`docs/UX.md` 6).
            constraints: BoxConstraints(minHeight: tokens.sizes.row),
            child: Padding(
              padding: EdgeInsets.symmetric(
                horizontal: tokens.spacing.x3,
                vertical: tokens.spacing.x1,
              ),
              child: Row(
                children: <Widget>[
                  HGlyphIcon(
                    HGlyph.hourglass,
                    size: HSize.glyph,
                    color: tokens.stateTextColor(HFlowState.held),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  Expanded(
                    child: Text(
                      sentence,
                      key: const Key('return-banner-sentence'),
                      style: tokens.typography.ui13.tinted(tokens.colors.fg0),
                    ),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  HButton(
                    key: const Key('return-banner-jump'),
                    onPressed: onJump,
                    child: Text(l10n.trayReturnJump),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  HButton(
                    key: const Key('return-banner-dismiss'),
                    variant: HButtonVariant.ghost,
                    onPressed: onDismiss,
                    child: Text(l10n.trayDismiss),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// An arrival from above: [HMotion.arriveOffset] of travel plus a fade, over
/// [HMotion.arrive] on [HMotion.enter] (`docs/UX.md` 2.2).
///
/// The same movement the rules screen builds for its own banner. It belongs
/// in `packages/ui`, where every screen could reach it; until the design
/// system is opened again, a screen that needs an arrival paints its own
/// rather than importing another feature (`docs/ARCHITECTURE.md` 5).
class ArriveFromTop extends StatefulWidget {
  /// Wraps [child].
  const ArriveFromTop({required this.child, super.key});

  /// What arrives.
  final Widget child;

  @override
  State<ArriveFromTop> createState() => _ArriveFromTopState();
}

class _ArriveFromTopState extends State<ArriveFromTop>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
  );
  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _controller,
    curve: HMotion.enter,
  );
  bool _done = false;

  @override
  void initState() {
    super.initState();
    _controller
      ..addStatusListener(_finish)
      ..forward();
  }

  void _finish(AnimationStatus status) {
    // The wrapper leaves the tree as soon as it has nothing left to do
    // (`docs/UX.md` 7).
    if (status == AnimationStatus.completed && mounted && !_done) {
      setState(() => _done = true);
    }
  }

  @override
  void dispose() {
    _curve.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_done) {
      return widget.child;
    }
    final double distance = HReducedMotion.distance(
      context,
      HMotion.arriveOffset,
    );
    final Widget faded = FadeTransition(opacity: _curve, child: widget.child);
    if (distance == 0) {
      return faded;
    }
    return AnimatedBuilder(
      animation: _curve,
      child: faded,
      builder: (BuildContext context, Widget? child) => Transform.translate(
        offset: Offset(0, -distance * (1 - _curve.value)),
        child: child,
      ),
    );
  }
}
