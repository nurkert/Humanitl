/// Notices that float over a region instead of standing in its column
/// (HUM-302).
///
/// Before this file every notice of the application took a band of its own:
/// the desktop notice and the return banner under the header of the shell,
/// the findings of the Sandbox above its terminal, the findings and the
/// requests of the agent above the queue. Each of them pushed what stood
/// below. The terminal lost rows and the TUI in it wrapped anew whenever a
/// finding came or went, and the queue jumped down by the height of a card.
///
/// One pattern replaces all of them:
///
/// * [HNoticeLayer] lays the notices **over** a region, top right, in a
///   [Stack]. The region keeps its full size whether a notice stands or not,
///   so nothing below moves and no terminal changes its size. The content is
///   always the first child of the stack and the notices always the second,
///   also while there is none: the element of the content survives every
///   arrival and every dismissal.
/// * [HNoticeCard] is the surface of one notice: `bg2`, a strong hairline,
///   the card radius and no shadow (`docs/DESIGN.md`, anti-reference 5). Its
///   close affordance [HNoticeDismiss] sits **inside** the card, top right,
///   a 28 px hit target with a label on hover. Only the first line keeps
///   clear of it ([noticeDismissClearance]); below, the text runs to the
///   inner edge.
/// * A diagnostic as a notice is an `HDiagnosticCard` with `dense: true` and
///   an [HNoticeDismiss] in its `dismiss` slot: the same corner, the same
///   button, the same surface.
///
/// A notice is passive. Nothing here takes the focus, binds a key or traps
/// the keyboard: no autofocus, no focus scope of its own, no `Shortcuts`.
/// The cards are ordinary focus stops after the content, reachable by `Tab`,
/// and `Esc` closes none of them, because `Esc` belongs to whatever the
/// person is doing below.
library;

import 'dart:math' as math;

import 'package:flutter/widgets.dart';

import 'hover_label.dart';
import 'ui.dart';

/// The widest a notice gets. Narrower regions get a card as wide as the
/// region minus its inset on both sides.
///
/// The width of an `HModal` (`docs/DESIGN.md`): the measure of a short
/// paragraph, not of a line across the window.
const double noticeMaxWidth = 420;

/// How far from the end edge of a card the first line of a notice stops:
/// the close affordance (4 px from the edge, 28 px of hit target and the
/// 2 px on each side its focus ring stands in) plus the 4 px gap of the
/// grid. Only the first line needs it; everything below the affordance takes
/// the full width of the card (HUM-302, review: no empty column on the
/// right).
const double noticeDismissClearance = HSpace.x1 + HSize.hitMin + HSpace.x2;

/// A region with notices floating over its top right corner.
class HNoticeLayer extends StatelessWidget {
  /// Lays [notices] over [child].
  const HNoticeLayer({
    required this.child,
    required this.notices,
    this.top,
    this.maxHeight,
    this.maxHeightFor,
    this.bottom = false,
    this.scroll = true,
    this.maxWidth = noticeMaxWidth,
    super.key,
  });

  /// The region. It keeps the full size of the layer at all times.
  final Widget child;

  /// The notices, one under the other. A widget that draws nothing while
  /// there is nothing to say; it then takes no space and catches no pointer.
  final Widget notices;

  /// The distance from the top edge of the region; the inset of the grid
  /// (`HSpace.x2`) by default. The shell passes the height of a pane header
  /// plus the inset, so a global notice never covers the controls in the
  /// header row of a pane (HUM-286).
  final double? top;

  /// The tallest the notices get before they scroll; by default the height
  /// of the region minus the insets.
  final double? maxHeight;

  /// Like [maxHeight], computed from the height of the region, for a caller
  /// whose bound depends on it (the Sandbox keeps the bottom rows of its
  /// terminal free). Applied after [maxHeight].
  final double Function(double regionHeight)? maxHeightFor;

  /// Whether the notices stand at the bottom right instead of the top right.
  /// The queue puts them there: its newest rows, the ones a person is about
  /// to decide, stand at the top and must stay clickable (HUM-302, review).
  final bool bottom;

  /// Whether the layer scrolls the notices itself. A caller whose notices
  /// scroll on their own (a `ListView` with its own scrollbar) passes false
  /// and gets a bounded height instead.
  final bool scroll;

  /// The widest a card gets.
  final double maxWidth;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final double inset = tokens.spacing.x2;
    final double top = this.top ?? inset;
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final double width = math.max(
          0,
          math.min(maxWidth, constraints.maxWidth - 2 * inset),
        );
        final double room = math.max(0, constraints.maxHeight - top - inset);
        final double? cap = maxHeight;
        final double Function(double)? capFor = maxHeightFor;
        double height = cap == null ? room : math.min(cap, room);
        if (capFor != null) {
          height = math.max(0, math.min(height, capFor(constraints.maxHeight)));
        }
        return Stack(
          fit: StackFit.expand,
          children: <Widget>[
            child,
            Positioned(
              key: const Key('notice-layer-notices'),
              top: bottom ? null : top,
              bottom: bottom ? inset : null,
              right: inset,
              width: width,
              // Ein Hinweis bleibt frei: Das schwebende Terminal legt sich
              // nicht über ihn (HUM-300). Ohne Hinweis ist die Fläche leer und
              // hält nichts frei.
              child: HKeepClear(
                child: ConstrainedBox(
                  constraints: BoxConstraints(maxHeight: height),
                  child: scroll
                      ? SingleChildScrollView(child: notices)
                      : notices,
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

/// The close affordance of a notice: a glyph inside the card, top right,
/// with a 28 px hit target and its label on hover.
class HNoticeDismiss extends StatelessWidget {
  /// Creates the affordance.
  const HNoticeDismiss({
    required this.label,
    required this.onPressed,
    this.buttonKey,
    super.key,
  });

  /// The label, already localised: read out and shown on hover.
  final String label;

  /// Closes the notice.
  final VoidCallback onPressed;

  /// The key of the button itself, for tests and focus checks.
  final Key? buttonKey;

  @override
  Widget build(BuildContext context) => HoverLabel(
    label: label,
    side: HoverLabelSide.below,
    child: HIconButton(
      key: buttonKey,
      glyph: HGlyph.close,
      semanticsLabel: label,
      onPressed: onPressed,
    ),
  );
}

/// The surface of one notice.
class HNoticeCard extends StatelessWidget {
  /// Creates a card around [child].
  const HNoticeCard({
    required this.header,
    required this.child,
    required this.dismissLabel,
    required this.onDismiss,
    this.rail,
    this.dismissKey,
    this.semanticsLabel,
    super.key,
  });

  /// The first line: it keeps clear of the close affordance.
  final Widget header;

  /// Everything below the first line, across the full width of the card.
  final Widget child;

  /// The label of the close affordance, already localised.
  final String dismissLabel;

  /// Closes the notice.
  final VoidCallback onDismiss;

  /// The colour of the state rail at the start edge, or none.
  final Color? rail;

  /// The key of the close button.
  final Key? dismissKey;

  /// What a screen reader reads for the card as a whole.
  final String? semanticsLabel;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color? rail = this.rail;
    final double start = rail == null
        ? tokens.spacing.x3
        : HSize.stateRail + tokens.spacing.x3;
    return Semantics(
      container: true,
      label: semanticsLabel,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.colors.bg2,
          borderRadius: BorderRadius.circular(tokens.radii.card),
          border: Border.all(color: tokens.colors.lineStrong),
        ),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(tokens.radii.card),
          // Die Schiene und der Knopf liegen im Stapel, damit die Schiene die
          // volle Höhe trägt und der Knopf in der Ecke steht, ohne dass der
          // Text seine Höhe schätzen muss; dasselbe Mittel wie in `HRow`.
          child: Stack(
            children: <Widget>[
              Padding(
                padding: EdgeInsetsDirectional.fromSTEB(
                  start,
                  tokens.spacing.x3,
                  tokens.spacing.x3,
                  tokens.spacing.x3,
                ),
                child: SizedBox(
                  width: double.infinity,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      // Nur die erste Zeile weicht dem Knopf aus; darunter
                      // gehört die ganze Breite dem Text.
                      Padding(
                        padding: EdgeInsetsDirectional.only(
                          end: noticeDismissClearance - tokens.spacing.x3,
                        ),
                        child: header,
                      ),
                      SizedBox(height: tokens.spacing.x2),
                      child,
                    ],
                  ),
                ),
              ),
              if (rail != null)
                PositionedDirectional(
                  top: 0,
                  bottom: 0,
                  start: 0,
                  width: HSize.stateRail,
                  child: ColoredBox(color: rail),
                ),
              PositionedDirectional(
                top: tokens.spacing.x1,
                end: tokens.spacing.x1,
                child: HNoticeDismiss(
                  label: dismissLabel,
                  onPressed: onDismiss,
                  buttonKey: dismissKey,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Notices one under the other, with the gap of the grid between them.
class HNoticeColumn extends StatelessWidget {
  /// Stacks [children]; draws nothing while there is none.
  const HNoticeColumn({required this.children, super.key});

  /// The notices, top first.
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    if (children.isEmpty) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final (int index, Widget child) in children.indexed) ...<Widget>[
          if (index > 0) SizedBox(height: tokens.spacing.x2),
          child,
        ],
      ],
    );
  }
}
