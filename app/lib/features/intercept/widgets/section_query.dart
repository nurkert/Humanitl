/// The query section of the request card: one row per parameter, monospace,
/// read-only.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/h_collapsible.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'key_value_table.dart';

/// The parameters of [pathAndQuery], in the order they appear.
///
/// Percent escapes are decoded where they can be; a value the decoder chokes
/// on is shown raw rather than dropped, because a malformed parameter is
/// exactly the kind of thing the person is looking at the card for.
List<KeyValue> parseQuery(String pathAndQuery) {
  final int mark = pathAndQuery.indexOf('?');
  if (mark < 0 || mark == pathAndQuery.length - 1) {
    return const <KeyValue>[];
  }
  final String query = pathAndQuery.substring(mark + 1);
  final List<KeyValue> parameters = <KeyValue>[];
  for (final String pair in query.split('&')) {
    if (pair.isEmpty) {
      continue;
    }
    final int equals = pair.indexOf('=');
    final String name = equals < 0 ? pair : pair.substring(0, equals);
    final String value = equals < 0 ? '' : pair.substring(equals + 1);
    parameters.add(KeyValue(_decode(name), _decode(value)));
  }
  return parameters;
}

String _decode(String raw) {
  try {
    return Uri.decodeQueryComponent(raw);
  } on ArgumentError {
    return raw;
  } on FormatException {
    return raw;
  }
}

/// The collapsible query section.
class SectionQuery extends StatelessWidget {
  /// Creates the section for [pathAndQuery].
  const SectionQuery({required this.pathAndQuery, super.key});

  /// The path with its query, as the daemon reported it.
  final String pathAndQuery;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    final HTokens tokens = HTheme.of(context);
    final List<KeyValue> parameters = parseQuery(pathAndQuery);
    return HCollapsible(
      title: l10n.interceptSectionQuery(parameters.length),
      child: parameters.isEmpty
          ? Text(
              l10n.interceptQueryEmpty,
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            )
          : KeyValueTable(rows: parameters),
    );
  }
}
