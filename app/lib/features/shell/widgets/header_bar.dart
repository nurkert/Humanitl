/// The 40 px header: wordmark, section title, intercept pill, hold count,
/// isolation ring and the palette button.
library;

import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/shortcuts/intents.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../../intercept/providers/flows.dart';
import '../../sandbox/providers/sandbox_status_provider.dart';
import '../../sandbox/sandbox_text.dart';
import '../section.dart';

/// The header.
///
/// A consumer rather than a plain widget so that the hold badge rebuilds on
/// its own: the count comes from [heldFlowsProvider], the very list the queue
/// pane draws, and reading it in the shell would rebuild every section on
/// every decision.
class HeaderBar extends ConsumerWidget {
  /// Creates the header for [section].
  const HeaderBar({required this.section, required this.onPalette, super.key});

  /// The shown section; its title sits next to the wordmark.
  final Section section;

  /// Opens the command palette.
  final VoidCallback onPalette;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final int heldCount = ref.watch(heldFlowsProvider).length;
    return SizedBox(
      height: tokens.sizes.headerBar,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.colors.bg1,
          border: Border(bottom: BorderSide(color: tokens.colors.line)),
        ),
        child: Padding(
          padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
          child: Row(
            children: <Widget>[
              const Wordmark(),
              SizedBox(width: tokens.spacing.x4),
              Text(
                section.label(l10n),
                key: const Key('header-section-title'),
                style: tokens.typography.ui13.medium.tinted(tokens.colors.fg1),
              ),
              const Spacer(),
              HBadge(text: l10n.shellInterceptOn, color: tokens.state.allowed),
              SizedBox(width: tokens.spacing.x2),
              HBadge(
                key: const Key('header-held-badge'),
                text: l10n.shellHeldCount(heldCount),
                color: heldCount > 0 ? tokens.state.held : tokens.colors.fg1,
              ),
              SizedBox(width: tokens.spacing.x3),
              const IsolationRing(),
              SizedBox(width: tokens.spacing.x3),
              HButton(
                key: const Key('header-palette-button'),
                variant: HButtonVariant.ghost,
                onPressed: onPalette,
                semanticsLabel: l10n.shellPaletteTitle,
                child: Text(l10n.shellPaletteHint),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The accent mark and the product name.
class Wordmark extends StatelessWidget {
  /// Creates the wordmark.
  const Wordmark({this.markSize = 16, this.style, super.key});

  /// Edge length of the accent mark.
  final double markSize;

  /// Text style of the name; 13/600 when null.
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        SizedBox.square(
          dimension: markSize,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: tokens.colors.accent,
              borderRadius: BorderRadius.circular(tokens.radii.control),
            ),
            child: Center(
              child: SizedBox(
                width: markSize * 0.375,
                height: markSize * 0.125,
                child: ColoredBox(color: tokens.colors.onAccent),
              ),
            ),
          ),
        ),
        SizedBox(width: tokens.spacing.x2),
        Text(
          context.l10n.appTitle,
          style:
              style ??
              tokens.typography.ui13.semibold.tinted(tokens.colors.fg0),
        ),
      ],
    );
  }
}

/// The 20 px isolation ring: one arc per guarantee, always in the header.
///
/// This is the product promise where it can always be seen (BACKLOG.md 5,
/// signature element 2). Each arc carries the state of one guarantee, and the
/// three states it can be in are the three the panel shows: grey when nothing
/// was measured, amber while the sandbox starts, green when the guarantee
/// holds and red when it does not. Grey is never a claim, and a closed green
/// ring is only ever three measured results.
///
/// A click goes to the sandbox section and opens the isolation tab, where the
/// evidence stands. It travels as a [NavIntent], the same way `Ctrl+4` does,
/// so there is exactly one way into a section.
class IsolationRing extends ConsumerStatefulWidget {
  /// Creates the ring.
  const IsolationRing({this.size = 20, super.key});

  /// Diameter of the ring.
  final double size;

  @override
  ConsumerState<IsolationRing> createState() => _IsolationRingState();
}

class _IsolationRingState extends ConsumerState<IsolationRing>
    with SingleTickerProviderStateMixin {
  late final AnimationController _breath = AnimationController(
    vsync: this,
    duration: HMotion.breathe,
  );

  /// The breath runs while a guarantee is being measured and stops with it:
  /// a flag, not a scale (`docs/UX.md` 2.7). Under reduced motion the amber
  /// stands still and says the same thing (2.10).
  void _syncBreath(bool measuring) {
    final bool wanted = measuring && !HReducedMotion.of(context);
    if (wanted && !_breath.isAnimating) {
      _breath.repeat(reverse: true);
    } else if (!wanted && _breath.isAnimating) {
      _breath
        ..stop()
        ..value = 0;
    }
  }

  @override
  void dispose() {
    _breath.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SandboxStatus status =
        ref.watch(sandboxStatusProvider).value ?? const SandboxStatus();
    final List<IsolationSegment> segments = <IsolationSegment>[
      for (final IsolationCheck check in IsolationCheck.values)
        status.segmentFor(check),
    ];
    _syncBreath(segments.contains(IsolationSegment.running));
    final IsolationCheckResult? failed = status.failedCheck;
    final Diagnostic? diagnostic = failed?.diagnostic;
    final String label = switch (failed) {
      null => l10n.shellIsolationPassed(
        status.checksPassed,
        IsolationCheck.values.length,
      ),
      _ => l10n.shellIsolationFailed(
        diagnostic == null || diagnostic.title.isEmpty
            ? isolationCheckSentence(l10n, failed.check)
            : diagnostic.title,
      ),
    };
    // Nothing measured is not a count of zero out of three: it is no answer
    // at all, and the label says that instead (CONVENTIONS 4.13).
    final String tooltip = status.checks.isEmpty
        ? l10n.shellIsolationUnknown
        : label;
    // An `HButton` and not a bare gesture: the ring is a control, so it takes
    // focus, answers Enter and Space, and shows a pressed state like every
    // other control (`docs/UX.md` 5.1). The label is the ghost variant's
    // child, which is the painted ring itself.
    return HoverLabel(
      label: tooltip,
      child: HButton(
        key: const Key('header-isolation-ring'),
        variant: HButtonVariant.ghost,
        semanticsLabel: tooltip,
        onPressed: () => _open(context),
        child: AnimatedBuilder(
          animation: _breath,
          builder: (BuildContext context, Widget? _) => CustomPaint(
            size: Size.square(widget.size),
            painter: _RingPainter(
              colours: <Color>[
                for (final IsolationSegment segment in segments)
                  isolationSegmentColor(tokens, segment),
              ],
              // Only the arcs that are being measured breathe; a guarantee
              // that is proven or refuted stands still, because its answer is
              // in.
              breathing: <bool>[
                for (final IsolationSegment segment in segments)
                  segment == IsolationSegment.running,
              ],
              breath: _breath.value,
            ),
          ),
        ),
      ),
    );
  }

  /// Shows the sandbox section with the isolation tab open.
  void _open(BuildContext context) {
    ref.read(sandboxTabChoiceProvider.notifier).go(SandboxTab.isolation);
    Actions.invoke(context, NavIntent(Section.sandbox.index));
  }
}

class _RingPainter extends CustomPainter {
  _RingPainter({
    required this.colours,
    required this.breathing,
    required this.breath,
  });

  /// One colour per guarantee, in the order of the panel.
  final List<Color> colours;

  /// Which arcs are being measured right now.
  final List<bool> breathing;

  /// Where in the breath the ring is, 0 to 1.
  final double breath;

  @override
  void paint(Canvas canvas, Size size) {
    final Paint paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.butt;
    final Rect rect = (Offset.zero & size).deflate(1);
    const double gap = 0.35;
    final double sweep = (2 * math.pi - 3 * gap) / colours.length;
    final double dim = 1 - breath * (1 - HMotion.breatheMinOpacity);
    for (int i = 0; i < colours.length; i++) {
      final double start = -math.pi / 2 + i * (sweep + gap);
      final Color colour = breathing[i]
          ? colours[i].withValues(alpha: colours[i].a * dim)
          : colours[i];
      canvas.drawArc(rect, start, sweep, false, paint..color = colour);
    }
  }

  @override
  bool shouldRepaint(_RingPainter oldDelegate) =>
      !listEquals(oldDelegate.colours, colours) ||
      !listEquals(oldDelegate.breathing, breathing) ||
      oldDelegate.breath != breath;
}
