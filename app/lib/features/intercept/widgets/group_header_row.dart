/// The header of a group of held requests (HUM-029).
///
/// One line, the same 36 px as every other row, and it keeps the row rule of
/// `docs/UX.md` 3.4: out of a row only blocking is possible. Allowing a whole
/// group is the widest-reaching act of this screen, so it goes through the
/// action bar, where the card beside it says what is about to leave
/// (`docs/UX.md` 3.5).
///
/// Clicking the header selects the group; the chevron folds it. Both have a
/// key: `Ctrl+A` selects the group of the cursor, `ArrowRight` and
/// `ArrowLeft` fold it (`docs/UX.md` 5.1).
library;

import 'dart:async';
import 'dart:math' as math;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/ui/hold_to_confirm.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/decision.dart';
import '../providers/held_groups.dart';
import 'countdown_ring.dart';

/// One group as a line.
class GroupHeaderRow extends ConsumerStatefulWidget {
  /// Creates the header of [group].
  const GroupHeaderRow({
    required this.group,
    required this.open,
    required this.selected,
    required this.onToggle,
    required this.onSelect,
    super.key,
  });

  /// The group this line stands for.
  final HeldGroup group;

  /// Whether the rows below it are shown.
  final bool open;

  /// True while every request of the group belongs to the selection.
  final bool selected;

  /// Folds the group.
  final VoidCallback onToggle;

  /// Selects every request of the group.
  final VoidCallback onSelect;

  @override
  ConsumerState<GroupHeaderRow> createState() => _GroupHeaderRowState();
}

class _GroupHeaderRowState extends ConsumerState<GroupHeaderRow> {
  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final HeldGroup group = widget.group;
    return HRow(
      key: Key('queue-group-${group.apex}'),
      state: HFlowState.held,
      tintedRail: true,
      inSelection: widget.selected,
      onTap: widget.onSelect,
      semanticsLabel: l10n.interceptGroupSummary(
        groupTitle(group, l10n),
        group.length,
        methodMix(group, l10n),
        l10n.interceptGroupFindings(group.findingsTotal),
      ),
      // The chevron stands first, where the eye starts the line, and the
      // countdown of the earliest deadline stands right, in the same column
      // as the countdowns of the rows underneath (HUM-029).
      leading: _Chevron(
        open: widget.open,
        label: l10n.interceptGroupToggle(groupTitle(group, l10n)),
        onToggle: widget.onToggle,
      ),
      title: ExcludeSemantics(
        child: Row(
          children: <Widget>[
            Flexible(
              child: Text(
                groupTitle(group, l10n),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
              ),
            ),
            SizedBox(width: tokens.spacing.x2),
            // A control that counts held requests stays accent: one can
            // touch it, so it is not a state display (`docs/UX.md` 3.3,
            // rule 8).
            HBadge(
              text: '${group.length}',
              color: tokens.colors.accent,
              semanticsLabel: l10n.interceptGroupCount(group.length),
            ),
            SizedBox(width: tokens.spacing.x2),
            Flexible(
              flex: 2,
              child: Text(
                methodMix(group, l10n),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tokens.typography.mono11.tinted(tokens.colors.fg1),
              ),
            ),
          ],
        ),
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (group.findingsTotal > 0) ...<Widget>[
            // The only chroma a resting queue line may carry
            // (`docs/UX.md` 4.7).
            HBadge(
              key: Key('queue-group-findings-${group.apex}'),
              text: '${group.findingsTotal}',
              color: tokens.state.error,
              semanticsLabel: l10n.interceptGroupFindings(group.findingsTotal),
            ),
            SizedBox(width: tokens.spacing.x2),
          ],
          CountdownLabel(flow: group.earliest),
        ],
      ),
      // Out of a row only blocking is possible; hover and focus uncover the
      // action in the slot (`docs/UX.md` 3.4, 3.5).
      actionSlot: _BlockGroup(group: group),
    );
  }
}

/// What a group is called on the screen.
///
/// One host is named; several are named by the first plus how many follow. The
/// registrable domain appears only where the daemon said it (the rule and its
/// sentence), never as a guess in a line that guards a decision
/// (`backlog/CONVENTIONS.md` 4.13).
String groupTitle(HeldGroup group, AppLocalizations l10n) =>
    group.display.isNotEmpty
    ? group.display
    : l10n.interceptGroupHosts(group.hosts.first, group.hosts.length - 1);

/// The method mix as one monospace line: `12× GET · 2× POST`.
String methodMix(HeldGroup group, AppLocalizations l10n) => group
    .methods
    .entries
    .map(
      (MapEntry<String, int> entry) =>
          l10n.interceptGroupMethod(entry.value, entry.key),
    )
    .join(l10n.interceptGroupSeparator);

/// The fold triangle. A pointer target with a key of its own.
class _Chevron extends StatelessWidget {
  const _Chevron({
    required this.open,
    required this.label,
    required this.onToggle,
  });

  final bool open;
  final String label;
  final VoidCallback onToggle;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Semantics(
      button: true,
      expanded: open,
      label: label,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: onToggle,
        child: SizedBox(
          width: HSize.hitMin,
          height: HSize.hitMin,
          child: Center(
            // A quarter turn is a state, not an event, so it does not animate
            // (`docs/UX.md` 2.2).
            child: Transform.rotate(
              angle: open ? math.pi / 2 : 0,
              child: HGlyphIcon(
                HGlyph.chevronRight,
                size: 14,
                color: tokens.colors.fg1,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// `Block {n}` in the action slot: the only decision a row offers.
///
/// Up to [modalAboveReach] requests the 250 ms hold is the protection; above
/// it the tap opens the modal that names the host and the consequence
/// (`docs/UX.md` 5.4).
class _BlockGroup extends ConsumerWidget {
  const _BlockGroup({required this.group});

  final HeldGroup group;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool asks = group.length > modalAboveReach;
    // One notifier decides, from the pointer as from the keyboard: what holds
    // for `Ctrl+Shift+L` holds for this control (ADR-018).
    // Out of a row, and the note of the action bar belongs to the selected
    // request, not to this group: it does not travel (HUM-072).
    void decide() => unawaited(
      ref
          .read(interceptDecisionProvider.notifier)
          .blockMany(group.flows, withNote: false),
    );
    // The glyph alone: the slot measures [HSize.rowActionSlot] and the number
    // it would block already stands in the counter chip of the same line. The
    // whole sentence -- "Block 12" -- is in the semantics label, which carries
    // it in every view (`docs/UX.md` 3.4, 6).
    final Widget content = HGlyphIcon(
      HGlyph.shieldX,
      size: 14,
      color: tokens.state.blocked,
    );
    // The row keeps the slot reserved and uncovers it on hover and on focus;
    // this is only what stands in it (`docs/UX.md` 3.4).
    return Semantics(
      container: true,
      button: true,
      label: l10n.interceptBlockGroup(group.length),
      child: SizedBox(
        height: HSize.rowActionSlot,
        child: asks
            ? GestureDetector(
                key: Key('queue-group-block-${group.apex}'),
                behavior: HitTestBehavior.opaque,
                onTap: decide,
                child: Center(child: content),
              )
            : HoldToConfirm(
                key: Key('queue-group-block-${group.apex}'),
                duration: HMotion.holdToBlock,
                fill: holdFill(tokens.state.blocked),
                // The hold belongs to this group as it stands: a request that
                // arrives or leaves cancels it (`docs/UX.md` 5.4).
                token: group.ids.join(','),
                onConfirmed: decide,
                onTapShort: () => ref
                    .read(lastRefusalProvider.notifier)
                    .refuse(RefusalReason.holdIt),
                builder: (BuildContext context, double progress) =>
                    Center(child: content),
              ),
      ),
    );
  }
}
