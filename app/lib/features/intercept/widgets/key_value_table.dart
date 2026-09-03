/// The two-column table the card uses for query parameters and headers.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// Width of the name column.
const double keyColumnWidth = 168;

/// One row of a [KeyValueTable].
@immutable
class KeyValue {
  /// Creates a row.
  const KeyValue(this.name, this.value);

  /// The name, shown in the left column.
  final String name;

  /// The value, shown in the right column.
  final String value;

  @override
  bool operator ==(Object other) =>
      other is KeyValue && other.name == name && other.value == value;

  @override
  int get hashCode => Object.hash(name, value);
}

/// A read-only table of names and values.
class KeyValueTable extends StatelessWidget {
  /// Creates a table of [rows]; [trailing] adds a control at the end of the
  /// row with the same index, for example the eye toggle of a masked header.
  const KeyValueTable({required this.rows, this.trailing, super.key});

  /// The rows, in order.
  final List<KeyValue> rows;

  /// Builds an optional control at the end of row [index].
  final Widget? Function(BuildContext context, int index)? trailing;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        for (int i = 0; i < rows.length; i++)
          Padding(
            padding: EdgeInsets.symmetric(vertical: tokens.spacing.x1 / 2),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                SizedBox(
                  width: keyColumnWidth,
                  child: Text(
                    rows[i].name,
                    style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                  ),
                ),
                SizedBox(width: tokens.spacing.x2),
                Expanded(
                  child: Text(
                    rows[i].value,
                    style: tokens.typography.mono12.tinted(tokens.colors.fg0),
                  ),
                ),
                if (trailing != null)
                  trailing!(context, i) ?? const SizedBox.shrink(),
              ],
            ),
          ),
      ],
    );
  }
}
