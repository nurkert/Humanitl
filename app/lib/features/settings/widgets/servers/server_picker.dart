/// A choice among the saved model servers, as a button that opens a menu
/// under itself (HUM-405).
///
/// `packages/ui` has no select; this is the context menu of the status bar
/// switch (HUM-289), opened by a click on a button that names the current
/// choice. The entry of the current choice stays in the menu, disabled, so
/// the menu keeps its order (`docs/UX.md` 5.2).
library;

import 'package:flutter/widgets.dart';

import '../../../../core/ui/ui.dart';

/// One choice of a [ServerPicker].
@immutable
class ServerPickerOption {
  /// A choice with [value] and the text [label].
  const ServerPickerOption({required this.value, required this.label});

  /// What is chosen; the empty text means "the default".
  final String value;

  /// What the menu and the button say.
  final String label;
}

/// The button with the current choice and the menu of [options].
class ServerPicker extends StatefulWidget {
  /// Creates a picker on [value].
  const ServerPicker({
    required this.value,
    required this.options,
    required this.onSelected,
    required this.semanticsLabel,
    this.enabled = true,
    super.key,
  });

  /// The current choice.
  final String value;

  /// The choices, in menu order.
  final List<ServerPickerOption> options;

  /// Called with the chosen value, never with the current one.
  final ValueChanged<String> onSelected;

  /// What a screen reader says for the button.
  final String semanticsLabel;

  /// Whether the menu opens.
  final bool enabled;

  @override
  State<ServerPicker> createState() => _ServerPickerState();
}

class _ServerPickerState extends State<ServerPicker> {
  final HContextMenuController _menu = HContextMenuController();
  final GlobalKey _anchor = GlobalKey();

  void _open() {
    final RenderObject? box = _anchor.currentContext?.findRenderObject();
    if (box is! RenderBox || !box.hasSize) {
      return;
    }
    _menu.open(box.localToGlobal(Offset(0, box.size.height + 2)));
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    String label = widget.value;
    for (final ServerPickerOption option in widget.options) {
      if (option.value == widget.value) {
        label = option.label;
      }
    }
    return HContextMenu(
      controller: _menu,
      semanticsLabel: widget.semanticsLabel,
      itemsBuilder: () => <HMenuItem>[
        for (final ServerPickerOption option in widget.options)
          HMenuItem(
            label: option.label,
            enabled: option.value != widget.value,
            onSelected: () => widget.onSelected(option.value),
          ),
      ],
      child: KeyedSubtree(
        key: _anchor,
        child: HButton(
          semanticsLabel: '${widget.semanticsLabel}: $label',
          onPressed: widget.enabled ? _open : null,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 200),
                child: Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tokens.typography.mono12,
                ),
              ),
              SizedBox(width: tokens.spacing.x2),
              RotatedBox(
                quarterTurns: 1,
                child: HGlyphIcon(
                  HGlyph.chevronRight,
                  size: 12,
                  color: tokens.colors.fg1,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
