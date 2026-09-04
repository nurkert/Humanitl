/// The queue as groups (HUM-029).
///
/// `npm install` fires fifteen requests in twenty seconds. A flat list turns
/// that into panic clicks; one line per registrable domain turns it into one
/// thing that can be read before it is decided. The grouping key is the apex,
/// not the host, so `registry.npmjs.org` and `codeload.github.com` do not
/// pretend to be unrelated to the domain they belong to.
///
/// Nothing here draws and nothing here reads a clock: a provider that groups
/// must never see the second hand, or an O(n log n) projection runs over the
/// whole session on every tick (`docs/UX.md` 7).
library;

import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../psl.dart';
import 'flows.dart';

part 'held_groups.g.dart';

/// From how many flows a shared apex is drawn as a group.
///
/// Below it the queue keeps the plain row of HUM-020: a header for a single
/// request would add a line without adding an answer.
const int groupFrom = 2;

/// From how many flows a group starts collapsed.
///
/// Two requests fit under their header without hiding anything; three or more
/// are the burst the group exists for.
const int collapseFrom = 3;

/// One apex with everything the queue holds for it.
@immutable
class HeldGroup {
  /// Creates a group. [flows] is in queue order, earliest deadline first.
  const HeldGroup({
    required this.apex,
    required this.display,
    required this.flows,
    required this.rows,
    required this.hosts,
    required this.methods,
    required this.findingsTotal,
    this.earliestDeadline,
  });

  /// The registrable domain the group is keyed by.
  final String apex;

  /// The one host of the group, or an empty string when it spans several.
  ///
  /// A group that spans two hosts is not named by the domain under them: that
  /// domain is a guess of `psl.dart`, and outside its table the guess can name
  /// a public suffix (`a.foo.com.pl` and `b.foo.com.pl` would become
  /// `com.pl`). Whoever draws the group says "host and n more" instead
  /// (`backlog/CONVENTIONS.md` 4.13).
  final String display;

  /// The held flows, earliest deadline first: everything a decision on this
  /// group covers, and everything the counter counts.
  ///
  /// A decided row rests in its place for its confirmation window (2.8), but
  /// it is not held any more: counting it would make the header claim a reach
  /// it does not have, and `Block {n}` would be refused for the whole group
  /// because one of them is already decided.
  final List<Flow> flows;

  /// Every line the group draws, the resting decided ones included.
  final List<Flow> rows;

  /// Every host under this apex, in the order they were met.
  final List<String> hosts;

  /// The method mix: label to count, the most frequent first.
  final Map<String, int> methods;

  /// The findings of every request in the group.
  final int findingsTotal;

  /// The deadline of the first flow that runs out, or null when none of them
  /// carries one.
  final DateTime? earliestDeadline;

  /// How many requests the group holds -- held ones only.
  int get length => flows.length;

  /// True when the group is drawn with a header of its own.
  bool get isBurst => flows.length >= groupFrom;

  /// Whether the group is open unless the person said otherwise.
  bool get openByDefault => flows.length < collapseFrom;

  /// The ids of the flows, in queue order.
  List<FlowId> get ids => <FlowId>[for (final Flow flow in flows) flow.id];

  /// The flow whose deadline runs out first, for the header countdown.
  Flow get earliest => flows.isEmpty ? rows.first : flows.first;

  @override
  bool operator ==(Object other) =>
      other is HeldGroup &&
      other.apex == apex &&
      other.display == display &&
      listEquals(other.flows, flows) &&
      listEquals(other.rows, rows) &&
      listEquals(other.hosts, hosts) &&
      mapEquals(other.methods, methods) &&
      other.findingsTotal == findingsTotal &&
      other.earliestDeadline == earliestDeadline;

  @override
  int get hashCode => Object.hash(
    apex,
    display,
    Object.hashAll(flows),
    Object.hashAll(rows),
    Object.hashAll(hosts),
    Object.hashAll(methods.entries.map((MapEntry<String, int> e) => e.key)),
    findingsTotal,
    earliestDeadline,
  );
}

/// The groups of a queue, as a type with value equality.
///
/// A bare `List` compares by identity, and a derived provider that returns one
/// notifies on every rebuild (`docs/UX.md` 7).
@immutable
class HeldGroups {
  /// Wraps [groups], in queue order.
  const HeldGroups(this.groups);

  /// No group at all.
  static const HeldGroups empty = HeldGroups(<HeldGroup>[]);

  /// The groups, the one with the earliest deadline first.
  final List<HeldGroup> groups;

  /// The group [id] belongs to, or null. Also finds a resting decided row.
  HeldGroup? groupOf(FlowId id) {
    for (final HeldGroup group in groups) {
      if (group.rows.any((Flow flow) => flow.id == id)) {
        return group;
      }
    }
    return null;
  }

  @override
  bool operator ==(Object other) =>
      other is HeldGroups && listEquals(groups, other.groups);

  @override
  int get hashCode => Object.hashAll(groups);
}

/// Groups [flows] by apex, keeping the order they come in.
///
/// The input is the queue order (earliest deadline first), so the first
/// appearance of an apex is also its earliest deadline: the groups come out
/// sorted without a second sort.
HeldGroups groupFlows(List<Flow> flows) {
  final Map<String, List<Flow>> byApex = <String, List<Flow>>{};
  for (final Flow flow in flows) {
    final String apex = apexOfHost(flow);
    (byApex[apex] ??= <Flow>[]).add(flow);
  }
  return HeldGroups(<HeldGroup>[
    for (final MapEntry<String, List<Flow>> entry in byApex.entries)
      _group(entry.key, entry.value),
  ]);
}

/// The registrable domain of [flow], from the bundled table.
///
/// The catalog of HUM-031 answers this from the public suffix list; until it
/// exists the table of [registrableDomain] stands in, in one place. It groups
/// the queue and nothing else: what a rule matches on comes from the daemon
/// (`selectedApexProvider`), never from this table
/// (`backlog/CONVENTIONS.md` 4.13).
String apexOfHost(Flow flow) =>
    registrableDomain(flow.host, isIpLiteral: flow.authority.isIpLiteral);

HeldGroup _group(String apex, List<Flow> rows) {
  // Everything below counts held requests only: the header says what a
  // decision on it would cover, and a decided row covers nothing any more.
  final List<Flow> flows = <Flow>[
    for (final Flow flow in rows)
      if (flow.isHeld) flow,
  ];
  final List<String> hosts = <String>[];
  final Map<String, int> counts = <String, int>{};
  int findings = 0;
  DateTime? earliest;
  for (final Flow flow in flows) {
    if (!hosts.contains(flow.host)) {
      hosts.add(flow.host);
    }
    counts[flow.methodLabel] = (counts[flow.methodLabel] ?? 0) + 1;
    findings += flow.findingCount;
    final DateTime? deadline = flow.deadline;
    if (deadline != null && (earliest == null || deadline.isBefore(earliest))) {
      earliest = deadline;
    }
  }
  final List<MapEntry<String, int>> ordered = counts.entries.toList()
    ..sort((MapEntry<String, int> a, MapEntry<String, int> b) {
      final int byCount = b.value.compareTo(a.value);
      return byCount != 0 ? byCount : a.key.compareTo(b.key);
    });
  return HeldGroup(
    apex: apex,
    display: hosts.length == 1 ? hosts.single : '',
    flows: flows,
    rows: rows,
    hosts: hosts,
    methods: <String, int>{
      for (final MapEntry<String, int> entry in ordered) entry.key: entry.value,
    },
    findingsTotal: findings,
    earliestDeadline: earliest,
  );
}

/// The held flows as groups, earliest deadline first.
///
/// Derived from [heldFlowsProvider], so a decision that leaves the queue also
/// leaves its group. The queue pane groups its own -- frozen -- list with
/// [groupFlows]; both go through the same function so the header and the rows
/// can never disagree.
@Riverpod(keepAlive: true)
HeldGroups heldGroups(Ref ref) => groupFlows(ref.watch(heldFlowsProvider));

/// Which groups the person opened or closed, for this session.
///
/// Only the deviations from [HeldGroup.openByDefault] are kept: the default
/// depends on how many requests a group holds, and that changes while the
/// person watches.
@Riverpod(keepAlive: true)
class ExpandedGroups extends _$ExpandedGroups {
  @override
  Map<String, bool> build() => const <String, bool>{};

  /// Whether [group] shows its rows.
  bool isOpen(HeldGroup group) => state[group.apex] ?? group.openByDefault;

  /// Opens a closed group and closes an open one.
  void toggle(HeldGroup group) => setOpen(group, !isOpen(group));

  /// Opens or closes [group]; the arrow keys say which of the two they mean.
  void setOpen(HeldGroup group, bool open) {
    if (isOpen(group) == open) {
      return;
    }
    state = <String, bool>{...state, group.apex: open};
  }
}
