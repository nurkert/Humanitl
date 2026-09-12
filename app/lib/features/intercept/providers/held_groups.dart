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
    required this.catalogId,
    required this.flows,
    required this.rows,
    required this.hosts,
    required this.methods,
    required this.findingsTotal,
    this.earliestDeadline,
  });

  /// The key of the group: the registrable domain the daemon named, or the
  /// host itself where it named none ([apexOfHost]).
  ///
  /// It is an identity, not a claim: nothing draws it as a domain, and the
  /// header names hosts (see [display]).
  final String apex;

  /// The one host of the group, or an empty string when it spans several.
  ///
  /// A group that spans two hosts is not named by the domain under them, even
  /// though the domain now comes from the daemon (`FlowSummary.apex`,
  /// HUM-091): whoever decides over several hosts has to read them, and the
  /// header that says "host and n more" is where they are counted
  /// (`backlog/CONVENTIONS.md` 4.13, "allowing is never easier than
  /// blocking"). The hosts themselves stand in [hosts].
  final String display;

  /// The catalog id every held flow of the group carries, or an empty string.
  ///
  /// It comes from the daemon, one answer per request
  /// (`FlowSummary.catalog_id`, HUM-094), and it is only kept where every held
  /// request of the group agrees on it. Two services under one header would be
  /// a name that covers a decision it does not describe, and a group without
  /// held requests carries none at all: a header is a claim about what a
  /// decision would reach (`backlog/CONVENTIONS.md` 4.13).
  final String catalogId;

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
      other.catalogId == catalogId &&
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
    catalogId,
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

/// The key [flow] is grouped under: the daemon's apex, or the host.
///
/// The registrable domain comes from the daemon, which carries the public
/// suffix list (`FlowSummary.apex`, HUM-091); the client never derives one.
/// An empty apex means the daemon does not know one — an IP literal, a host
/// that is itself a public suffix — and then the flow stands under its own
/// host. It is not filled in from a guess: `a.foo.com.pl` and `b.evil.com.pl`
/// are two registrants, and a table that put them under `com.pl` would draw
/// one decision over both (`backlog/CONVENTIONS.md` 4.13).
String apexOfHost(Flow flow) => flow.apex.isEmpty ? flow.host : flow.apex;

/// The one catalog id every flow of [flows] carries, or the empty string.
///
/// The empty string stands for all three ways of not knowing: no flow at all,
/// a flow the daemon named no service for, or two services under one header.
/// None of them may be filled in — the header of a group is read before a
/// decision that covers every request under it, and a name that fits only some
/// of them would make the decision look narrower than it is
/// (`backlog/CONVENTIONS.md` 4.13).
String sharedCatalogId(List<Flow> flows) {
  if (flows.isEmpty) {
    return '';
  }
  final String first = flows.first.catalogId;
  if (first.isEmpty) {
    return '';
  }
  return flows.every((Flow flow) => flow.catalogId == first) ? first : '';
}

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
    catalogId: sharedCatalogId(flows),
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
