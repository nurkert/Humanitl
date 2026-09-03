/// Text a person can select and copy, without pulling in Material.
///
/// The app builds on `WidgetsApp`, so `SelectableText` and `SelectionArea`
/// are out of reach; `SelectableRegion` is the widgets-layer equivalent. The
/// selection handles are the empty ones, which is right for a desktop window:
/// the pointer drags, `Ctrl+C` copies, nothing is dragged by a handle.
library;

import 'package:flutter/widgets.dart';

/// Selectable text in [style].
class SelectableMonoText extends StatelessWidget {
  /// Creates selectable text.
  const SelectableMonoText({
    required this.text,
    required this.style,
    this.maxLines,
    super.key,
  });

  /// What to show.
  final String text;

  /// How to show it.
  final TextStyle style;

  /// Cuts the text off after this many lines; null wraps without a limit.
  final int? maxLines;

  @override
  Widget build(BuildContext context) {
    return SelectableRegion(
      selectionControls: emptyTextSelectionControls,
      child: Text(text, style: style, maxLines: maxLines),
    );
  }
}
