/// The tab strip under the terminal (HUM-040).
///
/// A tab change is navigation, not an event: it happens in one frame, with no
/// crossfade and no travel (`docs/UX.md` 2.2). What moves is the two-pixel
/// mark under the active tab, and it moves because it says which of the four
/// is being read.
///
/// `app/packages/ui` has no tab component yet; this one is built out of its
/// parts -- `HControl` is not exported, so the strip stands on `HButton` in
/// its quiet variant plus the mark. See the report of HUM-040.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// One entry of the strip.
@immutable
class SandboxTabEntry<T> {
  /// A tab with [label] for [value].
  const SandboxTabEntry({required this.value, required this.label, this.key});

  /// What choosing it selects.
  final T value;

  /// The label, already localised.
  final String label;

  /// Identity of the control, for tests.
  final Key? key;
}

/// A row of tabs with a mark under the active one.
class SandboxTabs<T> extends StatelessWidget {
  /// Creates the strip.
  const SandboxTabs({
    required this.entries,
    required this.selected,
    required this.onSelect,
    super.key,
  });

  /// The tabs, left to right.
  final List<SandboxTabEntry<T>> entries;

  /// Which one is open.
  final T selected;

  /// Opens a tab.
  final ValueChanged<T> onSelect;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Container(
      color: tokens.colors.bg1,
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x2),
      child: Row(
        children: <Widget>[
          for (final SandboxTabEntry<T> entry in entries)
            _Tab<T>(
              entry: entry,
              active: entry.value == selected,
              onSelect: onSelect,
            ),
        ],
      ),
    );
  }
}

class _Tab<T> extends StatelessWidget {
  const _Tab({
    required this.entry,
    required this.active,
    required this.onSelect,
  });

  final SandboxTabEntry<T> entry;
  final bool active;
  final ValueChanged<T> onSelect;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    // The button sizes the stack; the mark is laid over its lower edge. It is
    // always in the tree and only its colour changes, so a tab change moves
    // nothing (`docs/UX.md` 2.9).
    return Stack(
      children: <Widget>[
        Padding(
          padding: EdgeInsets.only(bottom: tokens.spacing.x1),
          child: HButton(
            key: entry.key,
            variant: HButtonVariant.ghost,
            size: HButtonSize.sm,
            onPressed: () => onSelect(entry.value),
            child: Text(
              entry.label,
              style: active
                  ? tokens.typography.ui12.semibold.tinted(tokens.colors.fg0)
                  : tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ),
        ),
        Positioned(
          left: tokens.spacing.x2,
          right: tokens.spacing.x2,
          bottom: 0,
          height: HSize.selectionRail,
          child: HAnimatedFill(
            color: active ? tokens.colors.accent : tokens.colors.bg1,
            builder: (BuildContext context, Color color) =>
                ColoredBox(color: color),
          ),
        ),
      ],
    );
  }
}
