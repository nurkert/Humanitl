/// The return banner: what happened while the window was away (HUM-034).
///
/// The return plays nothing back. Fifteen arrivals that happened minutes ago
/// get no arrival animation, because an animation answers "what has just
/// arrived" and none of this just arrived (`docs/UX.md` 4.9). What is left is
/// one quiet line that names the longest wait and leads to the request behind
/// it.
///
/// Since HUM-302 the line is a card over the top right of the window
/// (`HNoticeLayer`), with its close affordance inside the card; the name
/// stays from the band it was before.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/h_notice_layer.dart';
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
      child: HNoticeCard(
        rail: tokens.state.held,
        semanticsLabel: sentence,
        dismissLabel: l10n.trayDismiss,
        dismissKey: const Key('return-banner-dismiss'),
        onDismiss: onDismiss,
        header: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
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
          ],
        ),
        child: Align(
          alignment: AlignmentDirectional.centerStart,
          child: HButton(
            key: const Key('return-banner-jump'),
            onPressed: onJump,
            child: Text(l10n.trayReturnJump),
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
