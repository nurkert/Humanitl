/// What the history asks the daemon for: the filter expression, the sort key
/// and whether passthrough traffic is included.
///
/// The query is a value; the answer to it lives in `history_page.dart`. The
/// filter expression is never parsed here. Its grammar belongs to the
/// recorder (`daemon/crates/recorder/src/filter.rs`), and an unknown key
/// comes back as a `Diagnostic` that names every valid key — showing that
/// answer teaches the grammar, rewriting it would hide it (`docs/UX.md` 4.4).
library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../history_metrics.dart';

/// One of the five quick filters above the table.
enum HistoryChip {
  /// Requests that are still waiting for a decision.
  held,

  /// Requests somebody or something refused.
  blocked,

  /// Requests the detectors found something in.
  findings,

  /// Requests that went out changed.
  edited,

  /// Traffic to the configured LLM endpoint, hidden by default.
  ///
  /// Not a filter term but the `include_passthrough` flag of `ListFlows`:
  /// passthrough flows are many, and a person who does not see them has to
  /// be told that they are hidden rather than left wondering where the model
  /// calls went (`backlog/sprint-2.md`, HUM-032, Fallstricke).
  passthrough;

  /// The filter term this chip adds, or null for [passthrough].
  String? get term => switch (this) {
    HistoryChip.held => 'state:held',
    HistoryChip.blocked => 'decision:block',
    HistoryChip.findings => 'findings:>0',
    HistoryChip.edited => 'edited:true',
    HistoryChip.passthrough => null,
  };
}

/// A history query: expression, order and scope.
@immutable
class HistoryQuery {
  /// Creates a query.
  const HistoryQuery({
    this.filter = '',
    this.sort = HistorySort.time,
    this.descending = true,
    this.includePassthrough = false,
  });

  /// Everything, newest first, without the LLM passthrough.
  static const HistoryQuery initial = HistoryQuery();

  /// The expression in the recorder's filter language, as typed.
  final String filter;

  /// Which column the rows are ordered by.
  final HistorySort sort;

  /// True for newest, largest or longest first.
  final bool descending;

  /// True when passthrough flows are listed as well.
  final bool includePassthrough;

  /// The value of `ListFlows.order_by`.
  ///
  /// The daemon reads the first word as the key and treats any later `asc` as
  /// ascending (`humanitl-ipc`, `order_of`); `desc` is spelled out so that
  /// the request says what it means.
  String get orderBy => '${sort.wire} ${descending ? 'desc' : 'asc'}';

  /// The wire filter of this query.
  FlowFilter get flowFilter => FlowFilter(
    query: filter,
    orderBy: orderBy,
    includePassthrough: includePassthrough,
  );

  /// True when nothing narrows the history but the passthrough default.
  bool get isUnfiltered => filter.trim().isEmpty;

  /// True when new arrivals belong at the top of this list.
  ///
  /// Only an unfiltered list ordered by arrival, newest first, can take a new
  /// flow without asking the daemon: for any other query only the recorder
  /// knows whether the arrival matches and where it would sit.
  bool get takesArrivals =>
      isUnfiltered && sort == HistorySort.time && descending;

  /// True when [chip] is on.
  bool has(HistoryChip chip) => chip == HistoryChip.passthrough
      ? includePassthrough
      : _terms.contains(chip.term);

  /// This query with [chip] turned on or off.
  HistoryQuery toggle(HistoryChip chip) {
    if (chip == HistoryChip.passthrough) {
      return copyWith(includePassthrough: !includePassthrough);
    }
    final String term = chip.term!;
    final List<String> terms = _terms;
    final List<String> next = terms.contains(term)
        ? (terms.toList()..remove(term))
        : <String>[...terms, term];
    return copyWith(filter: next.join(' '));
  }

  /// This query ordered by [sort]: a different column starts descending, the
  /// same column again flips the direction.
  HistoryQuery orderedBy(HistorySort sort) => this.sort == sort
      ? copyWith(descending: !descending)
      : copyWith(sort: sort, descending: true);

  /// A copy with the named fields replaced.
  HistoryQuery copyWith({
    String? filter,
    HistorySort? sort,
    bool? descending,
    bool? includePassthrough,
  }) => HistoryQuery(
    filter: filter ?? this.filter,
    sort: sort ?? this.sort,
    descending: descending ?? this.descending,
    includePassthrough: includePassthrough ?? this.includePassthrough,
  );

  /// The terms of [filter], split on whitespace outside quotes, the way the
  /// recorder tokenises them.
  List<String> get _terms {
    final List<String> terms = <String>[];
    final StringBuffer current = StringBuffer();
    bool quoted = false;
    bool started = false;
    for (final int rune in filter.runes) {
      final String char = String.fromCharCode(rune);
      if (char == '"') {
        quoted = !quoted;
        started = true;
        current.write(char);
      } else if (!quoted && char.trim().isEmpty) {
        if (started) {
          terms.add(current.toString());
          current.clear();
          started = false;
        }
      } else {
        started = true;
        current.write(char);
      }
    }
    if (started) {
      terms.add(current.toString());
    }
    return terms;
  }

  @override
  bool operator ==(Object other) =>
      other is HistoryQuery &&
      other.filter == filter &&
      other.sort == sort &&
      other.descending == descending &&
      other.includePassthrough == includePassthrough;

  @override
  int get hashCode => Object.hash(filter, sort, descending, includePassthrough);
}

/// The query the history screen currently shows.
///
/// Written by hand rather than generated: the canonical provider name of
/// `backlog/sprint-2.md` is `historyQueryProvider`, and the generator derives
/// the name from the notifier class, which would then have to be called
/// `HistoryQuery` as well. A hand-written `NotifierProvider` keeps the value
/// type's name and is kept alive by default, like every provider of this
/// feature.
final NotifierProvider<HistoryQueryNotifier, HistoryQuery>
historyQueryProvider = NotifierProvider<HistoryQueryNotifier, HistoryQuery>(
  HistoryQueryNotifier.new,
);

/// The notifier behind [historyQueryProvider].
class HistoryQueryNotifier extends Notifier<HistoryQuery> {
  @override
  HistoryQuery build() => HistoryQuery.initial;

  /// Takes the expression the person typed, trimmed of the surrounding blanks
  /// a paste brings with it.
  void submit(String filter) {
    final HistoryQuery next = state.copyWith(filter: filter.trim());
    if (next != state) {
      state = next;
    }
  }

  /// Turns [chip] on or off.
  void toggle(HistoryChip chip) => state = state.toggle(chip);

  /// Orders by [sort], flipping the direction when it is already the key.
  void orderBy(HistorySort sort) => state = state.orderedBy(sort);

  /// Back to everything, newest first, passthrough hidden.
  void reset() => state = HistoryQuery.initial;
}
