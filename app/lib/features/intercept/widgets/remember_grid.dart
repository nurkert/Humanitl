/// The remember grid: how long a decision holds, and what it covers.
///
/// Two segmented controls, and neither of them is filled: the one filled
/// control of the screen is the release valve (`docs/UX.md` 3.1). A chosen
/// segment carries the accent as a tint, an unchosen one carries nothing.
///
/// The grid is where a single decision becomes a rule, so it is never open by
/// default: `Enter` allows once and unchanged (BACKLOG.md 5). Choosing `Once`
/// greys the scope out -- there is no scope for a rule that is not created.
/// A single scope can be greyed out on its own: the registrable domain is
/// available only while the daemon has said what it is
/// (`backlog/CONVENTIONS.md` 4.13).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/focus_ring.dart';
import '../../../core/ui/ui.dart';
import '../rule_sentence.dart';

/// The two segmented controls.
class RememberGrid extends StatelessWidget {
  /// Creates the grid.
  const RememberGrid({
    required this.heading,
    required this.durationLabel,
    required this.targetLabel,
    required this.duration,
    required this.target,
    required this.durationLabels,
    required this.targetLabels,
    required this.onDuration,
    required this.onTarget,
    this.enabled = true,
    this.disabledTargets = const <RememberTarget>{},
    super.key,
  });

  /// The word in front of the two groups.
  final String heading;

  /// Screen-reader label of the duration group.
  final String durationLabel;

  /// Screen-reader label of the scope group.
  final String targetLabel;

  /// The chosen duration.
  final RememberDuration duration;

  /// The chosen scope.
  final RememberTarget target;

  /// The label of every duration segment, in enum order.
  final List<String> durationLabels;

  /// The label of every scope segment, in enum order.
  final List<String> targetLabels;

  /// Chooses a duration.
  final ValueChanged<RememberDuration> onDuration;

  /// Chooses a scope.
  final ValueChanged<RememberTarget> onTarget;

  /// False greys both groups out; nothing is decidable.
  final bool enabled;

  /// Scopes that cannot be chosen right now, greyed out one by one.
  final Set<RememberTarget> disabledTargets;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool scopeEnabled = enabled && duration != RememberDuration.once;
    // A `Wrap`, not a `Row`: at the minimum width of the pane, and at twice
    // the text scale, the two groups do not fit beside each other, and a bar
    // that overflows hides a control instead of moving it (`docs/UX.md` 6).
    return Wrap(
      crossAxisAlignment: WrapCrossAlignment.center,
      spacing: tokens.spacing.x3,
      runSpacing: tokens.spacing.x2,
      children: <Widget>[
        Text(heading, style: tokens.typography.ui12.tinted(tokens.colors.fg1)),
        _Segments(
          key: const Key('intercept-remember-duration'),
          groupLabel: durationLabel,
          labels: durationLabels,
          selected: duration.index,
          enabled: enabled,
          onSelect: (int index) => onDuration(RememberDuration.values[index]),
        ),
        _Segments(
          key: const Key('intercept-remember-target'),
          groupLabel: targetLabel,
          labels: targetLabels,
          selected: target.index,
          enabled: scopeEnabled,
          dimmed: <int>{
            for (final RememberTarget scope in disabledTargets) scope.index,
          },
          onSelect: (int index) => onTarget(RememberTarget.values[index]),
        ),
      ],
    );
  }
}

/// One segmented control: the segments in a hairline frame.
///
/// A `Wrap`, so that four segments at twice the text scale break into two
/// rows instead of overflowing the pane. The hairline between two segments is
/// the left border of the second one, which survives the break.
class _Segments extends StatelessWidget {
  const _Segments({
    required this.groupLabel,
    required this.labels,
    required this.selected,
    required this.enabled,
    required this.onSelect,
    this.dimmed = const <int>{},
    super.key,
  });

  final String groupLabel;
  final List<String> labels;
  final int selected;
  final bool enabled;

  /// Segments that are greyed out while the rest of the group is not.
  ///
  /// They stay clickable on purpose: the refusal belongs to the one place that
  /// knows the reason, and it says it out loud. A control that swallows a
  /// click in silence is the one thing `docs/UX.md` 5.3 forbids.
  final Set<int> dimmed;
  final ValueChanged<int> onSelect;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Semantics(
      container: true,
      label: groupLabel,
      child: Opacity(
        // The group as a whole fades; a single greyed out segment fades on its
        // own inside it.
        opacity: enabled ? 1 : 0.45,
        child: DecoratedBox(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(tokens.radii.control),
            border: Border.all(color: tokens.colors.line),
          ),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(tokens.radii.control),
            child: Wrap(
              children: <Widget>[
                for (int i = 0; i < labels.length; i++)
                  _Segment(
                    label: labels[i],
                    selected: i == selected,
                    enabled: enabled,
                    dimmed: dimmed.contains(i),
                    divided: i > 0,
                    onSelect: () => onSelect(i),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _Segment extends StatefulWidget {
  const _Segment({
    required this.label,
    required this.selected,
    required this.enabled,
    required this.divided,
    required this.onSelect,
    this.dimmed = false,
  });

  final String label;
  final bool selected;
  final bool enabled;

  /// Greyed out, but still reachable: choosing it is refused with a reason.
  final bool dimmed;

  /// True for every segment but the first: it carries the hairline that
  /// separates it from the one before.
  final bool divided;

  final VoidCallback onSelect;

  @override
  State<_Segment> createState() => _SegmentState();
}

class _SegmentState extends State<_Segment> {
  bool _focused = false;
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final double opacity = widget.enabled && !widget.dimmed ? 1 : 0.45;
    final Color background = widget.selected
        ? tokens.tint(tokens.colors.accent)
        : _hovered
        ? tokens.colors.bg2
        : const Color(0x00000000);
    return Opacity(
      opacity: opacity,
      child: FocusRing(
        visible: _focused,
        radius: tokens.radii.control,
        child: FocusableActionDetector(
          enabled: widget.enabled,
          mouseCursor: widget.enabled
              ? SystemMouseCursors.click
              : MouseCursor.defer,
          onFocusChange: (bool value) => setState(() => _focused = value),
          onShowHoverHighlight: (bool value) =>
              setState(() => _hovered = value),
          actions: <Type, Action<Intent>>{
            ActivateIntent: CallbackAction<ActivateIntent>(
              onInvoke: (ActivateIntent intent) {
                widget.onSelect();
                return null;
              },
            ),
          },
          child: Semantics(
            button: true,
            inMutuallyExclusiveGroup: true,
            selected: widget.selected,
            enabled: widget.enabled,
            label: widget.label,
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: widget.enabled ? widget.onSelect : null,
              child: AnimatedContainer(
                duration: HMotion.press,
                curve: HMotion.enter,
                constraints: const BoxConstraints(
                  minWidth: HSize.hitMin,
                  minHeight: HSize.hitMin,
                ),
                padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x2),
                decoration: BoxDecoration(
                  color: background,
                  border: widget.divided
                      ? Border(left: BorderSide(color: tokens.colors.line))
                      : null,
                ),
                child: Center(
                  widthFactor: 1,
                  child: ExcludeSemantics(
                    child: Text(
                      widget.label,
                      maxLines: 1,
                      style: tokens.typography.ui12.medium.tinted(
                        widget.selected ? tokens.colors.fg0 : tokens.colors.fg1,
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
